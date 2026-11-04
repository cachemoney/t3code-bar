import SwiftUI
import AppKit

public struct MenuBarIconView: View {
    public let status: ThreadStatus
    public let activeCount: Int

    public init(status: ThreadStatus, activeCount: Int) {
        self.status = status
        self.activeCount = activeCount
    }

    public var body: some View {
        HStack(spacing: 3) {
            Image(nsImage: MenuBarIconGenerator.createIcon(status: status, activeCount: activeCount))

            if status.showsLabelInMenuBar {
                Text(activeCount > 1 ? "\(status.label) \(activeCount)" : status.label)
                    .font(.system(size: 12, weight: .medium, design: .rounded))
                    .foregroundStyle(status.color)
            } else if activeCount > 1 {
                Text("\(activeCount)")
                    .font(.system(size: 12, weight: .medium, design: .rounded))
                    .foregroundStyle(Color.primary)
            }
        }
    }
}
