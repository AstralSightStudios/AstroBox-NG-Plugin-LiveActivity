import SwiftRs
import Tauri
import WebKit

private enum LiveActivityCommand: Sendable {
    case cleanupOrphans
    case create(CreateLiveActivityRequest)
    case update(UpdateLiveActivityRequest)
    case remove(RemoveLiveActivityRequest)
}

class LiveActivityPlugin: Plugin {
    private let commandContinuation: AsyncStream<LiveActivityCommand>.Continuation
    private var commandProcessor: Task<Void, Never>?

    override init() {
        let (stream, continuation) = AsyncStream.makeStream(of: LiveActivityCommand.self)
        self.commandContinuation = continuation
        super.init()

        // Tauri invokes native commands on its own serial queue, while
        // ActivityKit must be coordinated on the main actor. A single stream
        // consumer preserves create -> update -> remove ordering without
        // blocking the caller until a MainActor task completes.
        self.commandProcessor = Task { @MainActor in
            for await command in stream {
                guard !Task.isCancelled else { break }

                guard #available(iOS 16.2, *) else {
                    webviewLog("Live Activity is not supported on this system.")
                    continue
                }

                switch command {
                case .cleanupOrphans:
                    await ActivityManager.shared.endOrphanedActivities()
                case .create(let request):
                    await ActivityManager.shared.createActivity(with: request)
                case .update(let request):
                    await ActivityManager.shared.updateActivity(
                        activityId: request.activityId,
                        newState: request.state
                    )
                case .remove(let request):
                    await ActivityManager.shared.endActivity(activityId: request.activityId)
                }
            }
        }
    }

    deinit {
        commandContinuation.finish()
        commandProcessor?.cancel()
    }

    override func load(webview: WKWebView) {
        Task { @MainActor in
            WebViewLogger.shared.set(webview: webview)
        }
        // Cleanup is part of the same ordered command stream, so a new
        // activity cannot race an unfinished cold-launch cleanup.
        commandContinuation.yield(.cleanupOrphans)
    }

    @objc public func createLiveActivity(_ invoke: Invoke) throws {
        let args = try invoke.parseArgs(CreateLiveActivityRequest.self)
        commandContinuation.yield(.create(args))
        invoke.resolve()
    }

    @objc public func updateLiveActivity(_ invoke: Invoke) throws {
        let args = try invoke.parseArgs(UpdateLiveActivityRequest.self)
        commandContinuation.yield(.update(args))
        invoke.resolve()
    }

    @objc public func removeLiveActivity(_ invoke: Invoke) throws {
        let args = try invoke.parseArgs(RemoveLiveActivityRequest.self)
        commandContinuation.yield(.remove(args))
        invoke.resolve()
    }
}

@_cdecl("init_plugin_live_activity")
func initPlugin() -> Plugin {
    return LiveActivityPlugin()
}
