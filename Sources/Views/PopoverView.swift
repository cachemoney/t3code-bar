import SwiftUI

public struct PopoverView: View {
    @ObservedObject var monitor: ThreadMonitor
    @State private var contentHeight: CGFloat = 0

    private let onHeightChange: (CGFloat) -> Void

    public init(monitor: ThreadMonitor, onHeightChange: @escaping (CGFloat) -> Void = { _ in }) {
        self.monitor = monitor
        self.onHeightChange = onHeightChange
    }

    private var effectiveHeight: CGFloat {
        let maxHeight: CGFloat = 480
        if contentHeight > 0 {
            return min(contentHeight, maxHeight)
        }
        if monitor.visibleThreads.isEmpty {
            return 160
        }
        var total: CGFloat = 12
        for thread in monitor.visibleThreads {
            let hasBranch = !(thread.branch?.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty ?? true)
            total += (hasBranch ? 63 : 47) + 2
        }
        return min(max(total, 80), maxHeight)
    }

    public var body: some View {
        VStack(spacing: 0) {
            // Active Threads List (Pure view-only monitor, clean flat rows, no scrollbars)
            ScrollView(.vertical, showsIndicators: false) {
                VStack(spacing: 2) {
                    if monitor.isLoading {
                        ProgressView("Reading T3 Code data…")
                            .controlSize(.small)
                            .padding()
                    } else if let error = monitor.readError {
                        notice(error.localizedDescription, symbol: "exclamationmark.triangle")
                    } else if !monitor.isServerOnline {
                        notice("T3 Code offline · showing saved threads", symbol: "wifi.slash")
                    }
                    if !monitor.isLoading && monitor.visibleThreads.isEmpty && monitor.readError == nil {
                        VStack(spacing: 8) {
                            Image(systemName: "tray")
                                .font(.system(size: 20))
                                .foregroundColor(Color(hex: "#52525b"))
                            Text(monitor.searchQuery.isEmpty ? "No active threads" : "No matching threads")
                                .font(.system(size: 12))
                                .foregroundColor(Color(hex: "#71717a"))
                        }
                        .frame(maxWidth: .infinity)
                        .padding(.vertical, 32)
                    } else {
                        ForEach(monitor.visibleThreads) { thread in
                            ThreadRowView(thread: thread, showsLiveStatus: monitor.canShowLiveStatus) {
                                AppLauncher.openThread(thread: thread)
                            }
                        }
                    }
                }
                .padding(.horizontal, 6)
                .padding(.vertical, 6)
                .background(
                    GeometryReader { proxy in
                        Color.clear.preference(
                            key: ContentHeightPreferenceKey.self,
                            value: proxy.size.height
                        )
                    }
                )
            }
            .background(HideScrollerHelper())
        }
        .onPreferenceChange(ContentHeightPreferenceKey.self) { height in
            if height > 0 {
                contentHeight = height
                onHeightChange(effectiveHeight)
            }
        }
        .frame(width: 320)
        .frame(height: effectiveHeight)
        .animation(.easeInOut(duration: 0.15), value: effectiveHeight)
        .background(Color(hex: "#000000"))
        .preferredColorScheme(.dark)
    }

    private func notice(_ text: String, symbol: String) -> some View {
        VStack(alignment: .leading, spacing: 8) {
            Label(text, systemImage: symbol)
                .font(.system(size: 12))
                .foregroundStyle(.secondary)
                .fixedSize(horizontal: false, vertical: true)
            HStack {
                Button("Open T3 Code", action: AppLauncher.activateT3Code)
                Button("Retry", action: monitor.refresh)
            }
            .buttonStyle(.borderless)
            .font(.system(size: 12))
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(10)
    }
}

private struct ContentHeightPreferenceKey: PreferenceKey {
    static let defaultValue: CGFloat = 0
    static func reduce(value: inout CGFloat, nextValue: () -> CGFloat) {
        value = max(value, nextValue())
    }
}

private struct HideScrollerHelper: NSViewRepresentable {
    func makeNSView(context: Context) -> NSView {
        let view = NSView()
        DispatchQueue.main.async {
            if let scrollView = view.enclosingScrollView {
                scrollView.hasVerticalScroller = false
                scrollView.verticalScroller = nil
                scrollView.hasHorizontalScroller = false
                scrollView.horizontalScroller = nil
                scrollView.autohidesScrollers = true
            }
        }
        return view
    }

    func updateNSView(_ nsView: NSView, context: Context) {
        DispatchQueue.main.async {
            if let scrollView = nsView.enclosingScrollView {
                scrollView.hasVerticalScroller = false
                scrollView.verticalScroller = nil
            }
        }
    }
}
