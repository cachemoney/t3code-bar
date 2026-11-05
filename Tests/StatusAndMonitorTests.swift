import Foundation
import Testing
@testable import T3CodeBar

struct StatusAndMonitorTests {
    @Test func attentionOutranksWork() {
        #expect(makeThread(active: "running", approvals: 1, inputs: 1).status == .approval)
        #expect(makeThread(active: "running", inputs: 1).status == .input)
    }

    @Test(arguments: ["starting", "preparing"])
    func startupCountsAsWorking(_ status: String) {
        // T3 Code has no Connecting state: starting and preparing runs are Working.
        #expect(makeThread(active: status).status.label == "Working")
    }

    @Test func waitingGoalAndFailure() {
        #expect(makeThread(active: "waiting").status.label == "Working")
        #expect(makeThread(active: "running", goal: true).status.label == "Goal")
        #expect(makeThread(latest: "failed").status == .failed)
        #expect(makeThread(woke: true).status == .woke)
    }

    @Test func onlyUnseenCompletionsShowDoneWithoutArbitraryExpiry() {
        #expect(makeThread(latest: "completed").status == .done)
        #expect(makeThread(latest: "completed", lastVisitedAt: Date(timeIntervalSince1970: 1)).status == .settled)
        #expect(makeThread(latest: "completed", lastVisitedAt: Date(timeIntervalSince1970: 2)).status == .settled)
        #expect(makeThread(latest: "completed", lastVisitedAt: nil).status == .settled)
        #expect(makeThread().status == .settled)
    }

    @Test @MainActor func readFailurePreservesListButClearsLiveStatusAndRecovers() {
        let monitor = ThreadMonitor(startsMonitoring: false)
        let snapshot = ThreadSnapshot(threads: [makeThread(active: "running")], isServerOnline: true)
        monitor.apply(.success(snapshot))
        monitor.apply(.failure(.busy))
        #expect(monitor.activeThreads.count == 1)
        #expect(monitor.readError == .busy)
        #expect(!monitor.canShowLiveStatus)
        #expect(monitor.runningCount == 0)
        #expect(monitor.overallStatus == .settled)
        monitor.apply(.success(snapshot))
        #expect(monitor.readError == nil)
        #expect(monitor.canShowLiveStatus)
        #expect(monitor.runningCount == 1)
    }

    @Test @MainActor func offlineDataNeverAdvertisesLiveWork() {
        let monitor = ThreadMonitor(startsMonitoring: false)
        monitor.apply(.success(ThreadSnapshot(threads: [makeThread(active: "running", approvals: 1)], isServerOnline: false)))
        #expect(monitor.activeThreads.count == 1)
        #expect(monitor.attentionCount == 0)
        #expect(monitor.overallStatus == .settled)
    }
}

struct BadgeTests {
    @Test func badgeCountsThreadsInTheHighestPriorityState() {
        let running = [makeThread(id: "a", active: "running"), makeThread(id: "b", active: "running")]
        #expect(ThreadSummary(threads: running, isServerOnline: true).badgeCount == 2)

        let mixed = running + [makeThread(id: "c", approvals: 1)]
        let summary = ThreadSummary(threads: mixed, isServerOnline: true)
        #expect(summary.status == .approval)
        #expect(summary.badgeCount == 1)

        let twoInputs = running + [makeThread(id: "d", inputs: 1), makeThread(id: "e", inputs: 2)]
        let inputSummary = ThreadSummary(threads: twoInputs, isServerOnline: true)
        #expect(inputSummary.status == .input)
        #expect(inputSummary.badgeCount == 2)
    }
}

struct AttentionTrackerTests {
    @Test func alertsOnlyOnTransitionIntoAttention() {
        var tracker = AttentionTracker()
        #expect(tracker.update(with: [makeThread(id: "a", approvals: 1)]).isEmpty)
        #expect(tracker.update(with: [makeThread(id: "a", approvals: 1)]).isEmpty)

        let alerts = tracker.update(with: [makeThread(id: "a", approvals: 1), makeThread(id: "b", inputs: 1)])
        #expect(alerts.map(\.threadId) == ["b"])
        #expect(alerts.first?.status == .input)

        #expect(tracker.update(with: [makeThread(id: "a", active: "running"), makeThread(id: "b", inputs: 1)]).isEmpty)
        let again = tracker.update(with: [makeThread(id: "a", approvals: 1), makeThread(id: "b", inputs: 1)])
        #expect(again.map(\.threadId) == ["a"])
    }
}
