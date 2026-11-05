import SwiftUI

public struct ThreadRowView: View {
    public let thread: ThreadItem
    public let onSelect: () -> Void
    public let showsLiveStatus: Bool
    @State private var isHovered = false

    public init(thread: ThreadItem, showsLiveStatus: Bool = true, onSelect: @escaping () -> Void) {
        self.thread = thread
        self.showsLiveStatus = showsLiveStatus
        self.onSelect = onSelect
    }

    private func formatRelativeTime(_ date: Date) -> String {
        let seconds = max(0, Int(Date().timeIntervalSince(date)))
        if seconds < 60 {
            return "now"
        }
        let minutes = seconds / 60
        if minutes < 60 {
            return "\(minutes)m"
        }
        let hours = minutes / 60
        if hours < 24 {
            return "\(hours)h"
        }
        let days = hours / 24
        return "\(days)d"
    }

    /// How long until a future time, such as a usage limit reset: "45m", "2h", "3d".
    /// Nil once the time has passed, because then there is nothing left to wait for.
    nonisolated static func timeUntil(_ date: Date, now: Date = Date()) -> String? {
        let seconds = Int(date.timeIntervalSince(now).rounded(.up))
        guard seconds > 0 else { return nil }
        let minutes = max(1, (seconds + 59) / 60)
        if minutes < 60 { return "\(minutes)m" }
        let hours = (minutes + 59) / 60
        if hours < 24 { return "\(hours)h" }
        return "\((hours + 23) / 24)d"
    }

    /// One sentence for VoiceOver, so the row reads as a unit and status never depends on color.
    /// An idle thread has no status worth announcing.
    nonisolated static func accessibilityLabel(project: String, title: String, branch: String?, status: ThreadStatus) -> String {
        var parts = [title, "in \(project)"]
        if let branch, !branch.isEmpty { parts.append("on branch \(branch)") }
        if status != .settled { parts.append(status.label) }
        return parts.joined(separator: ", ")
    }

    private func pillSuffix(for status: ThreadStatus) -> String? {
        switch status {
        case .working(let started), .goal(let started):
            return started.map(formatDuration)
        case .waiting:
            return thread.latestRunCompletedAt.map(formatDuration)
        case .limited:
            return thread.usageLimitResetAt.flatMap { Self.timeUntil($0) }.map { "· \($0)" }
        default:
            return nil
        }
    }

    private func pillHelp(for status: ThreadStatus) -> String {
        switch status {
        case .done: return completionTooltip
        case .limited: return limitHelp
        case .request: return "A guest has a message waiting for you"
        default: return status.label
        }
    }

    private var limitHelp: String {
        if let reset = thread.usageLimitResetAt, let until = Self.timeUntil(reset) {
            return "Usage limit reached. Resets in \(until)."
        }
        return "Usage limit reached"
    }

    private func formatDuration(_ date: Date) -> String {
        let elapsed = max(0, Int(Date().timeIntervalSince(date)))
        if elapsed < 60 {
            return "\(elapsed)s"
        }
        let minutes = elapsed / 60
        if minutes < 60 {
            return "\(minutes)m"
        }
        let hours = minutes / 60
        let remMinutes = minutes % 60
        return "\(hours)h \(remMinutes)m"
    }

    private var hasGitBranch: Bool {
        guard let branch = thread.branch else { return false }
        let trimmed = branch.trimmingCharacters(in: .whitespacesAndNewlines)
        return !trimmed.isEmpty
    }

    private var completionDate: Date {
        thread.latestRunCompletedAt ?? thread.updatedAt
    }

    private var completionTooltip: String {
        "Done · \(completionDate.formatted(date: .abbreviated, time: .shortened))"
    }

    public var body: some View {
        Button(action: onSelect) {
            VStack(alignment: .leading, spacing: 3) {
            // Line 1: [Badge] [📌] [projectTitle] ... [Status or RelativeTime]
            HStack(alignment: .center, spacing: 6) {
                ProjectBadgeView(thread: thread)

                if thread.isPinned {
                    Image(systemName: "pin.fill")
                        .font(.system(size: 9))
                        .rotationEffect(.degrees(-45))
                        .foregroundColor(T3Theme.textMuted)
                }

                Text(thread.projectTitle)
                    .font(.system(size: 12.5, weight: .regular))
                    .foregroundColor(isHovered ? T3Theme.textHover : T3Theme.textSecondary)
                    .lineLimit(1)

                Spacer(minLength: 4)

                // Right slot: the dot and label T3 Code's sidebar uses, or the idle time.
                let status = showsLiveStatus ? thread.status : .settled
                if status == .settled {
                    Text(formatRelativeTime(thread.latestUserMessageAt ?? thread.updatedAt))
                        .font(.system(size: 10, weight: .regular))
                        .foregroundColor(T3Theme.textMuted)
                        .help(thread.latestRunStatus == "completed" && thread.latestRunCompletedAt != nil ? completionTooltip : "Last activity")
                } else {
                    StatusPill(status: status, suffix: pillSuffix(for: status))
                        .help(pillHelp(for: status))
                }
            }

            if hasGitBranch {
                // Thread with Git Branch: 3-line layout
                // Line 2: [Thread Title]
                Text(thread.title)
                    .font(.system(size: 13, weight: .medium))
                    .foregroundColor(isHovered ? Color.white : T3Theme.textPrimary)
                    .lineLimit(1)
                    .truncationMode(.tail)
                    .padding(.top, 1)

                // Line 3: [branch] ... [Provider Icon(s)]
                HStack(alignment: .center, spacing: 6) {
                    Text(thread.branch ?? "")
                        .font(.system(size: 11, weight: .regular))
                        .foregroundColor(isHovered ? T3Theme.textSecondary : T3Theme.textMuted)
                        .lineLimit(1)

                    Spacer()

                    ProviderIconView(providers: thread.providers, size: 13.5)
                }
                .padding(.top, 1)
            } else {
                // Thread without Git Branch: Compact 2-line layout (shorter row height like T3 Code)
                // Line 2: [Thread Title] ... [Provider Icon(s)]
                HStack(alignment: .center, spacing: 6) {
                    Text(thread.title)
                        .font(.system(size: 13, weight: .medium))
                        .foregroundColor(isHovered ? Color.white : T3Theme.textPrimary)
                        .lineLimit(1)
                        .truncationMode(.tail)

                    Spacer(minLength: 4)

                    ProviderIconView(providers: thread.providers, size: 13.5)
                }
                .padding(.top, 1)
            }
        }
        .padding(.horizontal, 10)
        .padding(.vertical, 7)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(
            RoundedRectangle(cornerRadius: 6, style: .continuous)
                .fill(isHovered ? T3Theme.rowHover : Color.clear)
        )
            .contentShape(RoundedRectangle(cornerRadius: 6, style: .continuous))
        }
        .buttonStyle(.plain)
        .focusable(false)
        .focusEffectDisabled()
        .accessibilityLabel(Self.accessibilityLabel(
            project: thread.projectTitle,
            title: thread.title,
            branch: hasGitBranch ? thread.branch : nil,
            status: showsLiveStatus ? thread.status : .settled
        ))
        .onHover { hovering in
            withAnimation(.easeInOut(duration: 0.12)) {
                isHovered = hovering
            }
            if hovering {
                NSCursor.pointingHand.push()
            } else {
                NSCursor.pop()
            }
        }
        .pointingHandCursor()
        .onDisappear {
            if isHovered {
                NSCursor.pop()
                isHovered = false
            }
        }
        .contextMenu {
            Button("Open in T3 Code") {
                AppLauncher.openThread(thread: thread)
            }
            Button("Open in Browser") {
                AppLauncher.openThreadInBrowser(threadId: thread.id)
            }
            Divider()
            Button("Copy Link") {
                AppLauncher.copyThreadLink(threadId: thread.id)
            }
        }
    }
}
