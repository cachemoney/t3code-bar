import Foundation
import Testing
@testable import T3CodeBar

struct StatusAndMonitorTests {
    @Test func attentionOutranksWork() {
        #expect(makeThread(active: "running", approvals: 1, inputs: 1).status == .approval)
        #expect(makeThread(active: "running", inputs: 1).status == .input)
        #expect(!ThreadStatus.approval.shouldPulse)
        #expect(!ThreadStatus.input.shouldPulse)
    }

    @Test(arguments: ["starting", "preparing"])
    func startupIsConnecting(_ status: String) {
        #expect(makeThread(active: status).status == .connecting)
    }

    @Test func waitingGoalPlanAndFailure() {
        #expect(makeThread(active: "waiting").status.label == "Working")
        #expect(makeThread(active: "running", goal: true).status.label == "Goal")
        #expect(makeThread(plan: true).status == .planReady)
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

    @Test @MainActor func searchDoesNotHideGlobalActivity() {
        let monitor = ThreadMonitor(startsMonitoring: false)
        monitor.apply(.success(ThreadSnapshot(threads: [makeThread(id: "visible"), makeThread(id: "hidden", active: "running")], isServerOnline: true)))
        monitor.searchQuery = "visible"
        #expect(monitor.visibleThreads.map(\.id) == ["visible"])
        #expect(monitor.runningCount == 1)
        #expect(monitor.overallStatus.label == "Working")
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
