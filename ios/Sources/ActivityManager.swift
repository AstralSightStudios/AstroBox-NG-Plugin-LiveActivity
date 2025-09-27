import Foundation
import SwiftUI
import ActivityKit

@available(iOS 16.2, *)
@MainActor
public final class ActivityManager {

    public static let shared = ActivityManager()

    private var currentActivity: Activity<LiveActivityAttributes>?

    /// 结束流程中的保护位，防止 end 期间又被 update 顶回去（iOS，很有意思吧）
    private var isEnding: Bool = false

    private init() {
        // 冷启动或 App 重启后，如果系统里还有正在进行的活动，则接管它（仅取第一个）
        self.currentActivity = Activity<LiveActivityAttributes>.activities.first
        if let act = self.currentActivity {
            webviewLog("Recovered existing live activity on init, ID: \(act.id)")
        }
    }

    // MARK: - Create

    /// 根据一个结构化的请求对象创建并启动一个新的实时活动。
    ///
    /// - Parameter request: 包含所有活动所需数据的 `CreateLiveActivityRequest` 对象。
    public func createActivity(with request: CreateLiveActivityRequest) {
        // 单实例：已有活动则拒绝创建
        // 因为我他妈的就是不想写多活动管理
        guard currentActivity == nil else {
            webviewLog("Error: A live activity already exists and cannot be created again.")
            return
        }

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
                let activity = try Activity.request(
                    attributes: attributes,
                    content: content,
                    pushType: nil
                )
                self.currentActivity = activity
                self.isEnding = false
                webviewLog("Successfully created live activity, ID: \(activity.id)")
            } catch {
                webviewLog("Error: Request to create live activity failed - \(error.localizedDescription)")
            }
        }
    }

    // MARK: - Update

    /// 更新当前实时活动的内容状态。
    /// - Parameter newState: 新的动态内容状态字典。
    public func updateActivity(newState: [String: String]) {
        guard !isEnding else {
            webviewLog("Skip update: activity is ending.")
            return
        }

        // 若内存丢失，尝试从系统找回唯一活动
        if currentActivity == nil {
            self.currentActivity = Activity<LiveActivityAttributes>.activities.first
            if let recovered = currentActivity {
                webviewLog("Recovered activity before update, ID: \(recovered.id)")
            }
        }

        guard let activity = currentActivity else {
            webviewLog("Note: There are no live activities in progress to update.")
            return
        }

        Task {
            let updatedContentState = LiveActivityAttributes.ContentState(stateItems: newState)
            let content = ActivityContent(state: updatedContentState, staleDate: nil)
            await activity.update(content)
            webviewLog("Live activity updated successfully.")
        }
    }

    // MARK: - End

    /// 结束当前实时活动（默认立刻回收）。
    ///
    /// - Parameters:
    ///   - finalState: (可选) 活动结束时显示的最终内容（建议带一个结束标记，方便 Widget 端切“完成”样式）
    ///   - dismissalPolicy: (可选) 结束策略，默认 `.immediate` 立刻回收
    public func endActivity(
        finalState: [String: String]? = nil,
        dismissalPolicy: ActivityUIDismissalPolicy = .immediate
    ) {
        // 若内存丢失，尝试从系统找回唯一活动
        if currentActivity == nil {
            self.currentActivity = Activity<LiveActivityAttributes>.activities.first
            if let recovered = currentActivity {
                webviewLog("Recovered activity for ending, ID: \(recovered.id)")
            }
        }

        guard let activity = currentActivity else {
            webviewLog("Note: There are no live activities in progress to end.")
            return
        }

        let finalContent: ActivityContent<LiveActivityAttributes.ContentState>?
        if let finalState {
            let finalContentState = LiveActivityAttributes.ContentState(stateItems: finalState)
            finalContent = ActivityContent(state: finalContentState, staleDate: nil)
        } else {
            finalContent = nil
        }

        isEnding = true

        Task {
            await activity.end(finalContent, dismissalPolicy: dismissalPolicy)
            // 结束后清空引用
            self.currentActivity = nil
            self.isEnding = false

            if Activity<LiveActivityAttributes>.activities.isEmpty {
                webviewLog("The live activity has ended (immediate).")
            } else {
                // 理论上单实例场景这里也应为空；如果非空，多数是系统宽限/可见性延迟
                webviewLog("The live activity requested to end; system may finalize shortly.")
            }
        }
    }
}
