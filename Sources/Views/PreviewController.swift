import SwiftUI

/// Decides when a row's preview appears. It shows only after the pointer has rested on the row
/// for `delay`, and any move away cancels it, so sweeping across the list never loads anything.
@MainActor
final class PreviewController: ObservableObject {
    static let defaultDelay: Duration = .milliseconds(500)

    @Published private(set) var preview: ThreadPreview?

    private let delay: Duration
    private let load: @MainActor (String) async -> ThreadPreview?
    private var task: Task<Void, Never>?

    init(delay: Duration = PreviewController.defaultDelay,
         load: @escaping @MainActor (String) async -> ThreadPreview?) {
        self.delay = delay
        self.load = load
    }

    func hover(_ hovering: Bool, threadId: String) {
        task?.cancel()
        task = nil
        guard hovering else {
            preview = nil
            return
        }
        task = Task { [weak self, delay, load] in
            try? await Task.sleep(for: delay)
            guard !Task.isCancelled else { return }
            let loaded = await load(threadId)
            // The pointer may have left while the read was running.
            guard !Task.isCancelled, let self else { return }
            self.preview = loaded
        }
    }
}

/// The expanded part of a row: an excerpt of the thread's last assistant reply.
struct PreviewBlock: View {
    let preview: ThreadPreview
    let age: String?

    var body: some View {
        VStack(alignment: .leading, spacing: 5) {
            Text(preview.text)
                .font(.system(size: 11.5))
                .lineSpacing(2)
                .foregroundColor(T3Theme.textSecondary)
                .fixedSize(horizontal: false, vertical: true)
                .frame(maxWidth: .infinity, alignment: .leading)
            if let age {
                Text("Last reply · \(age)")
                    .font(.system(size: 10))
                    .foregroundColor(T3Theme.textMuted)
            }
        }
        .padding(8)
        .background(RoundedRectangle(cornerRadius: 6, style: .continuous).fill(Color.white.opacity(0.05)))
        .padding(.top, 4)
        .accessibilityElement(children: .combine)
    }
}
