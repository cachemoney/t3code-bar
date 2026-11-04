import SwiftUI

public struct HeaderView: View {
    @Binding public var searchText: String
    public let onNewThread: () -> Void
    public let onActivateApp: () -> Void

    @State private var filterHovered: Bool = false
    @State private var folderHovered: Bool = false
    @State private var composeHovered: Bool = false

    public init(
        searchText: Binding<String>,
        onNewThread: @escaping () -> Void = { AppLauncher.activateT3Code() },
        onActivateApp: @escaping () -> Void = { AppLauncher.activateT3Code() }
    ) {
        self._searchText = searchText
        self.onNewThread = onNewThread
        self.onActivateApp = onActivateApp
    }

    public var body: some View {
        HStack(spacing: 8) {
            // Search Input Field
            HStack(spacing: 6) {
                Image(systemName: "magnifyingglass")
                    .font(.system(size: 12, weight: .medium))
                    .foregroundColor(Color(hex: "#71717a"))

                TextField("Search", text: $searchText)
                    .textFieldStyle(.plain)
                    .font(.system(size: 13, weight: .regular))
                    .foregroundColor(Color(hex: "#f4f4f5"))

                if !searchText.isEmpty {
                    Button(action: { searchText = "" }) {
                        Image(systemName: "xmark.circle.fill")
                            .font(.system(size: 11))
                            .foregroundColor(Color(hex: "#71717a"))
                    }
                    .buttonStyle(.plain)
                }
            }
            .padding(.horizontal, 8)
            .padding(.vertical, 5)
            .background(
                RoundedRectangle(cornerRadius: 6, style: .continuous)
                    .fill(Color(hex: "#121214"))
                    .overlay(
                        RoundedRectangle(cornerRadius: 6, style: .continuous)
                            .stroke(Color.white.opacity(0.12), lineWidth: 1)
                    )
            )

            // Right Action Icons
            HStack(spacing: 4) {
                // Filter Button
                Button(action: {}) {
                    Image(systemName: "line.3.horizontal.decrease")
                        .font(.system(size: 12, weight: .regular))
                        .foregroundColor(filterHovered ? Color.white : Color(hex: "#a1a1aa"))
                        .frame(width: 26, height: 26)
                        .background(filterHovered ? Color(hex: "#1e1e1e") : Color.clear)
                        .clipShape(RoundedRectangle(cornerRadius: 5))
                }
                .buttonStyle(.plain)
                .focusable(false)
                .onHover { hovering in
                    filterHovered = hovering
                    if hovering { NSCursor.pointingHand.push() } else { NSCursor.pop() }
                }
                .pointingHandCursor()
                .help("Filter")

                // Add Project Button
                Button(action: { onActivateApp() }) {
                    Image(systemName: "folder.badge.plus")
                        .font(.system(size: 12, weight: .regular))
                        .foregroundColor(folderHovered ? Color.white : Color(hex: "#a1a1aa"))
                        .frame(width: 26, height: 26)
                        .background(folderHovered ? Color(hex: "#1e1e1e") : Color.clear)
                        .clipShape(RoundedRectangle(cornerRadius: 5))
                }
                .buttonStyle(.plain)
                .focusable(false)
                .onHover { hovering in
                    folderHovered = hovering
                    if hovering { NSCursor.pointingHand.push() } else { NSCursor.pop() }
                }
                .pointingHandCursor()
                .help("Projects")

                // New Thread Button
                Button(action: {
                    AppLauncher.openNewThread()
                }) {
                    Image(systemName: "square.and.pencil")
                        .font(.system(size: 12.5, weight: .regular))
                        .foregroundColor(composeHovered ? Color.white : Color(hex: "#a1a1aa"))
                        .frame(width: 26, height: 26)
                        .background(composeHovered ? Color(hex: "#1e1e1e") : Color.clear)
                        .clipShape(RoundedRectangle(cornerRadius: 5))
                }
                .buttonStyle(.plain)
                .focusable(false)
                .onHover { hovering in
                    composeHovered = hovering
                    if hovering { NSCursor.pointingHand.push() } else { NSCursor.pop() }
                }
                .pointingHandCursor()
                .help("New Thread (⌘N)")
            }
        }
        .padding(.horizontal, 10)
        .padding(.vertical, 8)
    }
}
