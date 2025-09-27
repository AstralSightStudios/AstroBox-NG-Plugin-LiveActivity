import SwiftUI

#if os(iOS)
import UIKit
#elseif os(macOS)
import AppKit
#endif

/// 从本地绝对路径加载图片并转换为 SwiftUI Image
/// - Parameter path: 图片的绝对路径
/// - Returns: SwiftUI Image，如果加载失败则为 nil
public func imageFromAbsolutePath(_ path: String) -> Image? {
    #if os(iOS)
    if let uiImage = UIImage(contentsOfFile: path) {
        return Image(uiImage: uiImage)
    }
    #elseif os(macOS)
    if let nsImage = NSImage(contentsOfFile: path) {
        return Image(nsImage: nsImage)
    }
    #endif
    return nil
}
