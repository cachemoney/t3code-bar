import SwiftUI
import AppKit

// MARK: - Project Icon JSON Model

private struct ProjectIconPayload: Codable {
    let kind: String?
    let text: String?
    let color: String?
    let emoji: String?
    let name: String?
}

// MARK: - Favicon Cache & Resolver

public final class FaviconCache: @unchecked Sendable {
    public static let shared = FaviconCache()
    private var cache = [String: NSImage?]()
    private let lock = NSLock()

    private let candidates = [
        "favicon.svg",
        "favicon.ico",
        "favicon.png",
        "public/favicon.svg",
        "public/favicon.ico",
        "public/favicon.png",
        "app/favicon.ico",
        "app/favicon.png",
        "app/icon.svg",
        "app/icon.png",
        "app/icon.ico",
        "src/favicon.ico",
        "src/favicon.svg",
        "src/app/favicon.ico",
        "src/app/icon.svg",
        "src/app/icon.png",
        "assets/icon.svg",
        "assets/icon.png",
        "assets/logo.svg",
        "assets/logo.png",
        ".idea/icon.svg",
        "Resources/AppIcon.icns",
        "AppIcon.icns"
    ]

    public func resolve(workspaceRoot: String?, faviconPath: String?) -> NSImage? {
        let key = "\(faviconPath ?? "")\0\(workspaceRoot ?? "")"
        lock.lock()
        if let cached = cache[key] {
            lock.unlock()
            return cached
        }
        lock.unlock()

        let image = loadFavicon(workspaceRoot: workspaceRoot, faviconPath: faviconPath)
        lock.lock()
        cache[key] = image
        lock.unlock()
        return image
    }

    private func loadFavicon(workspaceRoot: String?, faviconPath: String?) -> NSImage? {
        let fm = FileManager.default
        if let fav = faviconPath, !fav.isEmpty {
            if fav.hasPrefix("/") && fm.fileExists(atPath: fav) {
                return NSImage(contentsOfFile: fav)
            }
            if let root = workspaceRoot, !root.isEmpty {
                let full = (root as NSString).appendingPathComponent(fav)
                if fm.fileExists(atPath: full) {
                    return NSImage(contentsOfFile: full)
                }
            }
        }
        guard let root = workspaceRoot, !root.isEmpty, fm.fileExists(atPath: root) else {
            return nil
        }
        for cand in candidates {
            let full = (root as NSString).appendingPathComponent(cand)
            if fm.fileExists(atPath: full) {
                if let img = NSImage(contentsOfFile: full) {
                    return img
                }
            }
        }
        return nil
    }
}

// MARK: - Lucide MessageSquareDashed Shape

public struct MessageSquareDashedShape: Shape {
    public init() {}

    public func path(in rect: CGRect) -> Path {
        var p = Path()
        let sx = rect.width / 24.0
        let sy = rect.height / 24.0
        func pt(_ x: CGFloat, _ y: CGFloat) -> CGPoint {
            CGPoint(x: rect.minX + x * sx, y: rect.minY + y * sy)
        }

        // Top border segments
        p.move(to: pt(9, 3)); p.addLine(to: pt(10, 3))
        p.move(to: pt(14, 3)); p.addLine(to: pt(15, 3))

        // Top right corner
        p.move(to: pt(18, 3))
        p.addQuadCurve(to: pt(21, 6), control: pt(21, 3))

        // Right border segments
        p.move(to: pt(21, 10)); p.addLine(to: pt(21, 11))
        p.move(to: pt(21, 15)); p.addLine(to: pt(21, 16))

        // Bottom right corner
        p.move(to: pt(21, 18))
        p.addQuadCurve(to: pt(18, 21), control: pt(21, 21))

        // Bottom border segment
        p.move(to: pt(14, 17)); p.addLine(to: pt(15, 17))

        // Bottom left corner + speech bubble tail
        p.move(to: pt(10, 17))
        p.addLine(to: pt(7, 17))
        p.addLine(to: pt(3, 21))
        p.addLine(to: pt(3, 6))
        p.addQuadCurve(to: pt(6, 3), control: pt(3, 3))

        return p
    }
}

// MARK: - ProjectBadgeView

public struct ProjectBadgeView: View {
    public let projectTitle: String
    public let projectIconJson: String?
    public let faviconPath: String?
    public let workspaceRoot: String?

    // Exact 18-color palette from T3 Code ProjectMonogram-DkfLdJyk.js
    public static let palette: [(name: String, hex: String)] = [
        ("gray", "#9ca3af"),
        ("red", "#f87171"),
        ("orange", "#fb923c"),
        ("amber", "#fbbf24"),
        ("yellow", "#facc15"),
        ("lime", "#a3e635"),
        ("green", "#4ade80"),
        ("emerald", "#34d399"),
        ("teal", "#2dd4bf"),
        ("cyan", "#22d3ee"),
        ("sky", "#38bdf8"),
        ("blue", "#60a5fa"),
        ("indigo", "#818cf8"),
        ("violet", "#a78bfa"),
        ("purple", "#c084fc"),
        ("fuchsia", "#e879f9"),
        ("pink", "#f472b6"),
        ("rose", "#fb7185")
    ]

    public init(
        projectTitle: String,
        projectIconJson: String? = nil,
        faviconPath: String? = nil,
        workspaceRoot: String? = nil
    ) {
        self.projectTitle = projectTitle
        self.projectIconJson = projectIconJson
        self.faviconPath = faviconPath
        self.workspaceRoot = workspaceRoot
    }

    public init(thread: ThreadItem) {
        self.init(
            projectTitle: thread.projectTitle,
            projectIconJson: thread.projectIconJson,
            faviconPath: thread.projectFaviconPath,
            workspaceRoot: thread.workspaceRoot
        )
    }

    // MARK: - Monogram Algorithm (exact port from T3 Code c(e))
    public static func extractMonogram(_ name: String) -> String {
        let clean = name.precomposedStringWithCanonicalMapping.trimmingCharacters(in: .whitespacesAndNewlines)
        guard let regex = try? NSRegularExpression(pattern: "[\\p{L}\\p{N}]+", options: []) else {
            return "PR"
        }
        let range = NSRange(clean.startIndex..<clean.endIndex, in: clean)
        let matches = regex.matches(in: clean, options: [], range: range)
        let words: [String] = matches.compactMap { match in
            guard let r = Range(match.range, in: clean) else { return nil }
            return String(clean[r])
        }
        guard let firstWord = words.first, !firstWord.isEmpty else {
            return "PR"
        }
        let firstChar = String(firstWord.prefix(1))

        var secondChar: String? = nil
        let remainder = firstWord.dropFirst()
        for ch in remainder {
            if ch.isNumber {
                secondChar = String(ch)
                break
            }
        }
        if secondChar == nil {
            if words.count > 1, let lastWord = words.last, let firstOfLast = lastWord.first {
                secondChar = String(firstOfLast)
            } else if let lastOfFirst = firstWord.last {
                secondChar = String(lastOfFirst)
            } else {
                secondChar = firstChar
            }
        }
        let combined = "\(firstChar)\(secondChar ?? firstChar)".uppercased()
        return String(combined.prefix(2))
    }

    // MARK: - Color Hash Algorithm (exact port from T3 Code l(e))
    public static func hashColor(for name: String) -> Color {
        let clean = name.precomposedStringWithCanonicalMapping.trimmingCharacters(in: .whitespacesAndNewlines)
        let lower = clean.lowercased(with: Locale(identifier: "en-US"))
        let target = lower.isEmpty ? "project" : lower
        var n = 0
        for scalar in target.unicodeScalars {
            n = (n * 31 + Int(scalar.value)) % palette.count
        }
        return Color(hex: palette[n].hex)
    }

    public static func colorByName(_ name: String?) -> Color {
        guard let name = name?.lowercased().trimmingCharacters(in: .whitespacesAndNewlines), !name.isEmpty else {
            return Color(hex: "#60a5fa")
        }
        if let found = palette.first(where: { $0.name == name }) {
            return Color(hex: found.hex)
        }
        return Color(hex: "#60a5fa")
    }

    private var parsedIcon: ProjectIconPayload? {
        guard let jsonStr = projectIconJson,
              let data = jsonStr.data(using: .utf8) else {
            return nil
        }
        return try? JSONDecoder().decode(ProjectIconPayload.self, from: data)
    }

    private var isNoProject: Bool {
        let lower = projectTitle.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
        return lower == "no project" || lower == "noproject"
    }

    public var body: some View {
        Group {
            if isNoProject || parsedIcon?.name == "message-square-dashed" {
                // "No project" case: message-square-dashed in gray
                MessageSquareDashedShape()
                    .stroke(
                        Self.colorByName(parsedIcon?.color ?? "gray"),
                        style: StrokeStyle(lineWidth: 1.4, lineCap: .round, lineJoin: .round)
                    )
                    .frame(width: 14, height: 14)
            } else if let icon = parsedIcon {
                switch icon.kind {
                case "monogram":
                    let text = icon.text ?? Self.extractMonogram(projectTitle)
                    let color = Self.colorByName(icon.color)
                    monogramBadge(text: text, color: color)

                case "emoji":
                    if let emoji = icon.emoji, !emoji.isEmpty {
                        Text(emoji)
                            .font(.system(size: 11))
                            .frame(width: 16, height: 16)
                    } else {
                        defaultBadge
                    }

                case "lucide":
                    if icon.name == "message-square-dashed" {
                        MessageSquareDashedShape()
                            .stroke(
                                Self.colorByName(icon.color ?? "gray"),
                                style: StrokeStyle(lineWidth: 1.4, lineCap: .round, lineJoin: .round)
                            )
                            .frame(width: 14, height: 14)
                    } else {
                        // Fallback SF Symbol for other lucide names
                        Image(systemName: "folder")
                            .font(.system(size: 11, weight: .medium))
                            .foregroundColor(Self.colorByName(icon.color))
                            .frame(width: 16, height: 16)
                    }

                default:
                    defaultBadge
                }
            } else {
                defaultBadge
            }
        }
        .frame(width: 16, height: 16)
    }

    @ViewBuilder
    private var defaultBadge: some View {
        if let favImg = FaviconCache.shared.resolve(workspaceRoot: workspaceRoot, faviconPath: faviconPath) {
            // Favicon case (e.g. Next.js N icon in gitshare-treemap)
            Image(nsImage: favImg)
                .resizable()
                .aspectRatio(contentMode: .fit)
                .frame(width: 16, height: 16)
                .clipShape(RoundedRectangle(cornerRadius: 4, style: .continuous))
        } else {
            // Auto monogram (e.g. I2 for in2, T3 for T3-Code-Bar)
            let text = Self.extractMonogram(projectTitle)
            let color = Self.hashColor(for: projectTitle)
            monogramBadge(text: text, color: color)
        }
    }

    private func monogramBadge(text: String, color: Color) -> some View {
        ZStack {
            RoundedRectangle(cornerRadius: 4, style: .continuous)
                .fill(color.opacity(0.14))

            Text(text)
                .font(.system(size: 8.25, weight: .bold, design: .monospaced))
                .foregroundColor(color)
                .tracking(text.count > 1 ? -0.4 : 0)
                .lineLimit(1)
        }
        .frame(width: 16, height: 16)
    }
}
