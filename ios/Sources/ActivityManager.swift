import Foundation
import SwiftUI
@preconcurrency import ActivityKit

@available(iOS 16.2, *)
@MainActor
public final class ActivityManager {

    public static let shared = ActivityManager()

    /// 当前存活的活动，按业务侧 activity id（即 attributes.id）索引。
    private var activitiesById: [String: Activity<LiveActivityAttributes>] = [:]

    /// 结束流程中的保护位（按 id），防止某个活动 end 期间又被 update 顶回去；互不影响其他活动。
    private var endingActivityIds: Set<String> = []

    private init() {
        // 冷启动或 App 重启后，接管系统里仍在进行的所有活动，按 id 归位。
        let existing = Activity<LiveActivityAttributes>.activities
        for act in existing {
            self.activitiesById[act.attributes.id] = act
            webviewLog("Recovered existing live activity on init, ID: \(act.attributes.id)")
        }
    }

    // MARK: - Create

    /// 根据一个结构化的请求对象创建并启动一个新的实时活动。
    ///
    /// - Parameter request: 包含所有活动所需数据的 `CreateLiveActivityRequest` 对象。
    public func createActivity(with request: CreateLiveActivityRequest) async {
        guard ActivityAuthorizationInfo().areActivitiesEnabled else {
            webviewLog("Tip: The user has disabled live activity in the system.")
            return
        }

        webviewLog("Processing live activity creation request with version \(request.activityContentV)...")

        switch request.activityContent {
        case .taskQueue(let taskQueueData):
            let attributes = LiveActivityAttributes(
                id: taskQueueData.id,
                type: LiveActivityContent.ContentType.taskQueue,
                title: taskQueueData.title,
                text: taskQueueData.text,
                taskName: taskQueueData.taskName,
                taskType: taskQueueData.taskType,
                taskIcon: taskQueueData.taskIcon,
            )

            let contentState = LiveActivityAttributes.ContentState(
                stateItems: taskQueueData.state
            )

            let content = ActivityContent(state: contentState, staleDate: nil)

            do {
                // 同 id 重复创建：先结束旧的活动再请求新的，保证同 id 只有一个实例。
                if let oldActivity = self.activitiesById[taskQueueData.id] {
                    webviewLog("Duplicate live activity ID \(taskQueueData.id), ending the previous one first.")
                    self.endingActivityIds.insert(taskQueueData.id)
                    self.activitiesById[taskQueueData.id] = nil
                    await oldActivity.end(nil, dismissalPolicy: .immediate)
                    self.endingActivityIds.remove(taskQueueData.id)
                }

                let activity = try Activity.request(
                    attributes: attributes,
                    content: content,
                    pushType: nil
                )
                self.activitiesById[activity.attributes.id] = activity
                webviewLog("Successfully created live activity, ID: \(activity.attributes.id)")
            } catch {
                webviewLog("Error: Request to create live activity failed - \(error.localizedDescription)")
            }
        }
    }

    // MARK: - Cold-launch cleanup

    /// 进程冷启动时调用：任务队列是进程内状态，不会跨进程存活，所以系统里若还残留着
    /// 实时活动，它一定是“孤儿”——对应任务已随上一个进程（被系统/用户杀掉）一起消失。
    ///
    /// 由于被杀的 App 不会再执行任何代码（不会回调 willTerminate），唯一能可靠清理这种
    /// 残留灵动岛/锁屏活动的时机就是下一次冷启动。这里把它们全部立刻结束。
    public func endOrphanedActivities() async {
        let activities = Activity<LiveActivityAttributes>.activities

        // 先就地清空本地记录，让随后的新建请求不被旧状态影响。
        activitiesById.removeAll()
        endingActivityIds.removeAll()

        guard !activities.isEmpty else {
            return
        }

        webviewLog("Cold launch: ending \(activities.count) orphaned live activity(ies).")

        for activity in activities {
            await activity.end(nil, dismissalPolicy: .immediate)
        }
        webviewLog("Cold launch: orphaned live activities ended.")
    }

    // MARK: - Update

    /// 更新指定实时活动的内容状态。
    /// - Parameters:
    ///   - activityId: 目标活动的业务 id（attributes.id）。
    ///   - newState: 新的动态内容状态字典。
    public func updateActivity(activityId: String, newState: [String: String]) async {
        guard !endingActivityIds.contains(activityId) else {
            webviewLog("Skip update: activity \(activityId) is ending.")
            return
        }

        if activitiesById[activityId] == nil {
            // 本地记录缺失时以 ActivityKit 为准做一次恢复。
            if let recovered = Activity<LiveActivityAttributes>.activities.first(where: { $0.attributes.id == activityId }) {
                activitiesById[activityId] = recovered
                webviewLog("Recovered activity before update, ID: \(activityId)")
            }
        }

        // Operate on the exact stored instance; re-querying ActivityKit by
        // attributes.id could grab a stale duplicate during replacement.
        guard let activity = activitiesById[activityId] else {
            webviewLog("Note: No live activity with ID \(activityId) in progress to update.")
            return
        }

        let updatedContentState = LiveActivityAttributes.ContentState(stateItems: newState)
        let content = ActivityContent(state: updatedContentState, staleDate: nil)

        await activity.update(content)

        webviewLog("Live activity \(activityId) updated successfully.")
    }

    // MARK: - End

    /// 结束指定的实时活动（默认立刻回收）。
    ///
    /// - Parameters:
    ///   - activityId: 目标活动的业务 id（attributes.id）。
    ///   - finalState: (可选) 活动结束时显示的最终内容（建议带一个结束标记，方便 Widget 端切“完成”样式）
    ///   - dismissalPolicy: (可选) 结束策略，默认 `.immediate` 立刻回收
    public func endActivity(
        activityId: String,
        finalState: [String: String]? = nil,
        dismissalPolicy: ActivityUIDismissalPolicy = .immediate
    ) async {
        if activitiesById[activityId] == nil {
            // 本地记录缺失时以 ActivityKit 为准做一次恢复。
            if let recovered = Activity<LiveActivityAttributes>.activities.first(where: { $0.attributes.id == activityId }) {
                activitiesById[activityId] = recovered
                webviewLog("Recovered activity for ending, ID: \(activityId)")
            }
        }

        endingActivityIds.insert(activityId)

        // End the exact stored instance (never a re-queried duplicate).
        guard let activity = activitiesById[activityId] else {
            endingActivityIds.remove(activityId)
            webviewLog("Note: No live activity with ID \(activityId) in progress to end.")
            return
        }

        let finalContent: ActivityContent<LiveActivityAttributes.ContentState>?

        if let finalState {
            let finalContentState = LiveActivityAttributes.ContentState(stateItems: finalState)
            finalContent = ActivityContent(state: finalContentState, staleDate: nil)
        } else {
            finalContent = nil
        }

        await activity.end(finalContent, dismissalPolicy: dismissalPolicy)

        // Only clear the mapping if it still points at the instance
        // we just ended; a newer instance may have taken over this id.
        if self.activitiesById[activityId] === activity {
            self.activitiesById[activityId] = nil
        }
        self.endingActivityIds.remove(activityId)

        if Activity<LiveActivityAttributes>.activities.isEmpty {
            webviewLog("The live activity \(activityId) has ended (immediate).")
        } else {
            webviewLog("The live activity \(activityId) requested to end; system may finalize shortly.")
        }
    }
}
