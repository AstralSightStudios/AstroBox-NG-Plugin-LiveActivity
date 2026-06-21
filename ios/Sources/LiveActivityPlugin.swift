import SwiftRs
import Tauri
import WebKit

class LiveActivityPlugin: Plugin {
    override func load(webview: WKWebView) {
        Task { @MainActor in
            WebViewLogger.shared.set(webview: webview)
            // 冷启动清理：上次进程若被杀，残留的实时活动是孤儿，直接清掉。
            if #available(iOS 16.2, *) {
                ActivityManager.shared.endOrphanedActivities()
            }
        }
    }
    
    @objc public func createLiveActivity(_ invoke: Invoke) throws {
        let args = try invoke.parseArgs(CreateLiveActivityRequest.self)
        if #available(iOS 16.2, *) {
          webviewLog("Joining main thread...")
          Task { @MainActor in
              webviewLog("Creating activity...")
              ActivityManager.shared.createActivity(with: args)
          }
        } else {
          webviewLog("Live Activity Unsupport this system.")
          // 不支持的系统什么都不做
        }
        invoke.resolve()
    }
    
    @objc public func updateLiveActivity(_ invoke: Invoke) throws {
        let args = try invoke.parseArgs(UpdateLiveActivityRequest.self)
        if #available(iOS 16.2, *) {
            Task { @MainActor in
                ActivityManager.shared.updateActivity(newState: args.state)
            }
        }
        invoke.resolve()
    }
    
    @objc public func removeLiveActivity(_ invoke: Invoke) throws {
        if #available(iOS 16.2, *) {
            Task { @MainActor in
                ActivityManager.shared.endActivity()
            }
        }
        invoke.resolve()
    }
}

@_cdecl("init_plugin_live_activity")
func initPlugin() -> Plugin {
    return LiveActivityPlugin()
}
