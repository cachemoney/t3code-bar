import SwiftUI
import AppKit

/// The Lucide icon T3 Code's sidebar shows beside each thread status.
/// SVGs live in `Sources/Resources/Lucide-*.svg` (ISC license, see LUCIDE-LICENSE.txt).
enum StatusIcon: String, CaseIterable {
    case working = "circle-dashed"
    case waiting = "circle-dot-dashed"
    case approval = "shield-question-mark"
    case input = "message-circle-question-mark"
    case request = "user-round"
    case failed = "circle-alert"
    case woke = "alarm-clock"
    case done = "circle-check"

    var resourceName: String { "Lucide-\(rawValue)" }

    @MainActor private static var cache: [StatusIcon: NSImage] = [:]

    /// Template image so SwiftUI can tint it; nil only if the resource is missing.
    @MainActor var image: NSImage? {
        if let cached = Self.cache[self] { return cached }
        guard let image = AppResources.image(named: resourceName, withExtension: "svg") else { return nil }
        image.isTemplate = true
        Self.cache[self] = image
        return image
    }
}

extension ThreadStatus {
    /// Which icon T3 Code uses for this status. Idle threads show their age instead.
    var icon: StatusIcon? {
        switch self {
        case .working, .goal: return .working
        case .waiting: return .waiting
        case .approval: return .approval
        case .input: return .input
        case .request: return .request
        case .failed, .limited: return .failed
        case .woke: return .woke
        case .done: return .done
        case .settled: return nil
        }
    }
}

/// T3 Code's sidebar status: a 14pt Lucide icon and a medium-weight label in one color,
/// with an optional trailing duration.
struct StatusPill: View {
    let status: ThreadStatus
    var suffix: String?

    var body: some View {
        HStack(spacing: 4) {
            if let image = status.icon?.image {
                Image(nsImage: image)
                    .renderingMode(.template)
                    .resizable()
                    .aspectRatio(contentMode: .fit)
                    .frame(width: 14, height: 14)
                    .accessibilityHidden(true)
            }
            Text(status.label)
            if let suffix {
                Text(suffix).monospacedDigit()
            }
        }
        .font(.system(size: 12, weight: .medium))
        .foregroundColor(status.color)
        .lineLimit(1)
        .fixedSize()
    }
}
