import WebKit

class WebViewLogger {
    @MainActor static let shared = WebViewLogger()
    
    @MainActor private static var webview: WKWebView?

    private init() {}
    
    @MainActor public func set(webview: WKWebView) {
        WebViewLogger.webview = webview
        self.log("Logger initialized and webview is captured.")
    }

    @MainActor public func log(_ message: String) {
        guard let webview = WebViewLogger.webview else {
            print("[Swift Plugin] Logger Error: WebView is not set. Message: \(message)")
            return
        }

        let escapedMessage = message.replacingOccurrences(of: "`", with: "\\`")
                                     .replacingOccurrences(of: "$", with: "\\$")

        let javascript = "console.log(`[Swift Plugin] \(escapedMessage)`);"
        
        DispatchQueue.main.async {
            webview.evaluateJavaScript(javascript) { (result, error) in
                if let error = error {
                    print("[Swift Plugin] Failed to execute JavaScript: \(error)")
                }
            }
        }
    }
}

public func webviewLog(_ message: String) {
    Task { @MainActor in
        WebViewLogger.shared.log(message)
    }
}
