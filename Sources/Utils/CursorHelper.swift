import Foundation
import AppKit
import SwiftUI

public struct PointingHandModifier: ViewModifier {
    public init() {}

    public func body(content: Content) -> some View {
        if #available(macOS 15.0, *) {
            content
                .pointerStyle(.link)
        } else {
            content
        }
    }
}

public extension View {
    func pointingHandCursor() -> some View {
        modifier(PointingHandModifier())
    }
}
