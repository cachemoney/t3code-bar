import Foundation

/// What a row shows after the pointer rests on it: the start of the thread's last assistant reply.
public struct ThreadPreview: Equatable, Sendable {
    public let text: String
    public let updatedAt: Date?

    public init(text: String, updatedAt: Date?) {
        self.text = text
        self.updatedAt = updatedAt
    }
}

/// Whether hover previews are shown. Previews show real message text, so they can be switched off
/// from the right-click menu (for example while sharing a screen).
public enum PreviewSettings {
    public static let key = "showPreviews"

    public static var isEnabled: Bool {
        UserDefaults.standard.object(forKey: key) as? Bool ?? true
    }
}

public enum PreviewText {
    public static let maxLines = 5
    public static let maxCharacters = 360

    /// A short plain-text excerpt of a markdown reply: code blocks dropped, markers removed,
    /// blank lines collapsed, then cut to `maxLines` lines and `maxCharacters` characters.
    /// Empty when nothing readable is left, such as a reply that is only code.
    public static func excerpt(_ markdown: String) -> String {
        var text = markdown.replacingOccurrences(
            of: "```[\\s\\S]*?(```|$)", with: "", options: .regularExpression)
        text = text.replacingOccurrences(
            of: "\\[([^\\]]+)\\]\\([^)]*\\)", with: "$1", options: .regularExpression)

        var lines: [String] = []
        for raw in text.split(separator: "\n", omittingEmptySubsequences: true) {
            var line = raw.trimmingCharacters(in: .whitespaces)
            line = line.replacingOccurrences(of: "^#{1,6}\\s+", with: "", options: .regularExpression)
            line = line.replacingOccurrences(of: "^>\\s?", with: "", options: .regularExpression)
            line = line.replacingOccurrences(of: "^[-*]\\s+", with: "• ", options: .regularExpression)
            for marker in ["**", "__", "`"] { line = line.replacingOccurrences(of: marker, with: "") }
            line = line.trimmingCharacters(in: .whitespaces)
            if !line.isEmpty { lines.append(line) }
        }
        guard !lines.isEmpty else { return "" }

        var truncated = lines.count > maxLines
        var result = lines.prefix(maxLines).joined(separator: "\n")
        if result.count > maxCharacters {
            let cut = result.prefix(maxCharacters)
            // Cut at a word boundary so the ellipsis does not follow half a word.
            result = String(cut[..<(cut.lastIndex(of: " ") ?? cut.endIndex)])
            truncated = true
        }
        return truncated ? result + "…" : result
    }
}
