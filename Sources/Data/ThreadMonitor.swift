import Foundation
import Combine

public struct ThreadSummary: Equatable, Sendable {
    public var status: ThreadStatus = .settled
    public var attentionCount = 0
    public var runningCount = 0
    public var badgeCount = 0

    public init(threads: [ThreadItem], isServerOnline: Bool) {
        guard isServerOnline else { return }
        for thread in threads {
            let current = thread.status
            if current.priority > status.priority { status = current }
            if current.isAttention { attentionCount += 1 }
            switch current {
            case .working, .goal: runningCount += 1
            default: break
            }
        }
        if status.isRunning {
            badgeCount = runningCount
        } else if status.isAttention {
            badgeCount = threads.filter { $0.status.priority == status.priority }.count
        }
    }
}

@MainActor
public final class ThreadMonitor: ObservableObject {
    @Published public private(set) var activeThreads: [ThreadItem] = []
    @Published public private(set) var isServerOnline = false
    @Published public private(set) var isLoading = true
    @Published public private(set) var readError: DatabaseReadError?
    @Published public private(set) var summary = ThreadSummary(threads: [], isServerOnline: false)
    @Published public private(set) var lastRefreshedAt: Date?
    /// Set after every read, failed or not, so the UI can tell a retry has finished.
    @Published public private(set) var lastAttemptAt: Date?

    public var overallStatus: ThreadStatus { summary.status }
    public var attentionCount: Int { summary.attentionCount }
    public var runningCount: Int { summary.runningCount }
    public var badgeCount: Int { summary.badgeCount }
    public var canShowLiveStatus: Bool { isServerOnline && readError == nil }

    private let database: DatabaseManager
    private var timer: Timer?
    private var refreshTask: Task<Void, Never>?

    public init(database: DatabaseManager = .shared, startsMonitoring: Bool = true) {
        self.database = database
        if startsMonitoring {
            refresh()
            startMonitoring()
        }
    }

    public func startMonitoring() {
        timer?.invalidate()
        timer = Timer.scheduledTimer(withTimeInterval: 1, repeats: true) { [weak self] _ in
            Task { @MainActor [weak self] in self?.refresh() }
        }
    }

    public func stopMonitoring() {
        timer?.invalidate()
        timer = nil
        refreshTask?.cancel()
        refreshTask = nil
    }

    public func refresh() {
        // Slow reads never enqueue overlapping polls. Cancelled results cannot update the UI.
        guard refreshTask == nil else { return }
        refreshTask = Task { [weak self, database] in
            let result: Result<ThreadSnapshot, DatabaseReadError>
            do {
                result = .success(try await database.snapshot())
            } catch {
                result = .failure(error as? DatabaseReadError ?? .unavailable)
            }
            guard !Task.isCancelled, let self else { return }
            self.apply(result)
            self.refreshTask = nil
        }
    }

    /// A preview is a convenience, so a failed read is simply no preview.
    public func preview(for threadId: String) async -> ThreadPreview? {
        try? await database.preview(threadId: threadId)
    }

    func apply(_ result: Result<ThreadSnapshot, DatabaseReadError>) {
        isLoading = false
        lastAttemptAt = Date()
        switch result {
        case .success(let snapshot):
            activeThreads = snapshot.threads
            isServerOnline = snapshot.isServerOnline
            readError = nil
            lastRefreshedAt = Date()
            summary = ThreadSummary(threads: snapshot.threads, isServerOnline: snapshot.isServerOnline)
        case .failure(let error):
            // Preserve the last successful list, but never present its activity as current.
            readError = error
            isServerOnline = false
            summary = ThreadSummary(threads: [], isServerOnline: false)
        }
    }
}
