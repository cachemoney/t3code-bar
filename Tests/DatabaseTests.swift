import Foundation
import Testing
@testable import T3CodeBar

struct DatabaseTests {
    @Test func moreThanFiftyThreadsRetainActivityAndPinOrder() async throws {
        let fixture = try DatabaseFixture()
        for index in 0..<55 { try fixture.thread(String(format: "t%03d", index)) }
        try fixture.thread("pinned-z", payload: #"{"pinnedAt":"2026-11-04","pinOrderKey":"z"}"#)
        try fixture.thread("pinned-b", payload: #"{"pinnedAt":"2026-11-04","pinOrderKey":"b"}"#)
        try fixture.run("t054", status: "running")
        try fixture.execute("INSERT INTO orchestration_v2_projection_runtime_requests VALUES ('t053', 'pending', 'approval')")
        let threads = try await DatabaseManager(config: fixture.config).fetchActiveThreads()
        #expect(threads.count == 57)
        #expect(Array(threads.prefix(2).map(\.id)) == ["pinned-b", "pinned-z"])
        #expect(threads.first { $0.id == "t054" }?.status.label == "Working")
        let summary = ThreadSummary(threads: threads, isServerOnline: true)
        #expect(summary.runningCount == 1)
        #expect(summary.status == .approval)
    }

    @Test func silentAndUnstartedCancelledRunsDoNotReplaceVisibleOutcome() async throws {
        let fixture = try DatabaseFixture()
        try fixture.thread("t", payload: #"{"lastVisitedAt":"2026-11-04T00:00:00Z"}"#)
        try fixture.run("t", status: "completed", payload: #"{"startedAt":"2026-11-04T00:00:01Z"}"#, completedAt: "2026-11-04T00:00:02Z")
        try fixture.run("t", status: "running", ordinal: 2, payload: #"{"silent":true}"#)
        try fixture.run("t", status: "cancelled", ordinal: 3)
        try fixture.run("t", status: "queued", ordinal: 4)
        let threads = try await DatabaseManager(config: fixture.config).fetchActiveThreads()
        let thread = try #require(threads.first)
        #expect(thread.activeRunStatus == nil)
        #expect(thread.status == .done)
        #expect(thread.latestRunStartedAt == ISO8601DateFormatter().date(from: "2026-11-04T00:00:01Z"))
    }

    @Test func hiddenThreadsStayOutOfInbox() async throws {
        let fixture = try DatabaseFixture()
        try fixture.thread("visible")
        try fixture.thread("settled", payload: #"{"settledAt":"2026-11-04T00:00:00Z"}"#)
        try fixture.thread("snoozed", payload: #"{"snoozedUntil":"2099-01-01T00:00:00Z"}"#)
        try fixture.thread("child", payload: #"{"lineage":{"relationshipToParent":"subagent"}}"#)
        try fixture.thread("side", payload: #"{"lineage":{"sideChat":true}}"#)
        try fixture.thread("archived")
        try fixture.execute("UPDATE orchestration_v2_projection_threads SET archived_at='2026-11-04' WHERE thread_id='archived'")
        let threads = try await DatabaseManager(config: fixture.config).fetchActiveThreads()
        #expect(threads.map(\.id) == ["visible"])
    }

    @Test(arguments: ["approval", "user_input"])
    func pendingRequestsWakeSnoozedThreads(_ kind: String) async throws {
        let fixture = try DatabaseFixture()
        try fixture.thread("t", payload: #"{"snoozedAt":"2026-11-04T00:00:00Z","snoozedUntil":"2099-01-01T00:00:00Z"}"#)
        try fixture.execute("INSERT INTO orchestration_v2_projection_runtime_requests VALUES ('t', 'pending', ?)", values: [kind])
        let database = DatabaseManager(config: fixture.config)
        let threads = try await database.fetchActiveThreads()
        let thread = try #require(threads.first)
        #expect(!thread.isSnoozed)
        #expect(thread.status == (kind == "approval" ? .approval : .input))
        // Resolved requests no longer override the still-future snooze.
        try fixture.execute("UPDATE orchestration_v2_projection_runtime_requests SET status='resolved'")
        #expect(try await database.fetchActiveThreads().isEmpty)
    }

    @Test(arguments: ["2026-11-03T23:59:59Z", "2026-11-04T00:00:00Z", "2026-11-04T00:00:01Z"])
    func onlyCompletionsAfterSnoozingWakeEarly(_ completedAt: String) async throws {
        let fixture = try DatabaseFixture()
        try fixture.thread("t", payload: #"{"snoozedAt":"2026-11-04T00:00:00Z","snoozedUntil":"2099-01-01T00:00:00Z","lastVisitedAt":"2026-11-03T00:00:00Z"}"#)
        try fixture.run("t", status: "completed", completedAt: completedAt)
        let threads = try await DatabaseManager(config: fixture.config).fetchActiveThreads()
        if completedAt > "2026-11-04T00:00:00Z" {
            #expect(threads.first?.status == .woke)
            #expect(threads.first?.isSnoozed == false)
        } else {
            #expect(threads.isEmpty)
        }
    }

    @Test func visitAcknowledgesEarlyWakeEvenAfterScheduledTime() async throws {
        let fixture = try DatabaseFixture()
        try fixture.thread("t", payload: #"{"snoozedAt":"2020-01-01T00:00:00Z","snoozedUntil":"2020-01-02T00:00:00Z","lastVisitedAt":"2020-01-01T00:02:00Z"}"#)
        try fixture.run("t", status: "completed", completedAt: "2020-01-01T00:01:00Z")
        let threads = try await DatabaseManager(config: fixture.config).fetchActiveThreads()
        #expect(threads.first?.isWoke == false)
        #expect(threads.first?.status == .settled)
    }

    @Test(arguments: ["2019-12-31T23:59:59Z", "2020-01-01T00:00:00Z", "2020-01-01T00:00:01Z"])
    func timerWakeUsesPersistedVisit(_ visit: String) async throws {
        let fixture = try DatabaseFixture()
        let payload = #"{"snoozedUntil":"2020-01-01T00:00:00Z","lastVisitedAt":"\#(visit)"}"#
        try fixture.thread("t", payload: payload)
        let threads = try await DatabaseManager(config: fixture.config).fetchActiveThreads()
        #expect(threads.first?.isWoke == (visit < "2020-01-01T00:00:00Z"))
    }

    @Test(arguments: ["2019-12-31T23:59:59Z", "2020-01-01T00:00:00Z", "2020-01-01T00:00:01Z"])
    func onlyFreshSessionErrorsWakeEarly(_ failedAt: String) async throws {
        let fixture = try DatabaseFixture()
        try fixture.thread("t", payload: #"{"snoozedAt":"2020-01-01T00:00:00Z","snoozedUntil":"2099-01-01T00:00:00Z"}"#)
        let session = #"{"status":"error","updatedAt":"\#(failedAt)"}"#
        try fixture.execute("INSERT INTO orchestration_v2_projection_provider_threads VALUES ('t', ?, 1)", values: [session])
        let threads = try await DatabaseManager(config: fixture.config).fetchActiveThreads()
        #expect(threads.isEmpty == (failedAt <= "2020-01-01T00:00:00Z"))
    }

    @Test func invalidSnoozeDateNeverHidesThread() async throws {
        let fixture = try DatabaseFixture()
        try fixture.thread("t", payload: #"{"snoozedUntil":"invalid"}"#)
        let threads = try await DatabaseManager(config: fixture.config).fetchActiveThreads()
        #expect(threads.count == 1)
        #expect(threads.first?.isWoke == false)
    }

    @Test func missingDatabaseIsNotCreated() async throws {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        let config = EnvironmentConfig(stateDirectory: directory)
        await #expect(throws: DatabaseReadError.missing) {
            try await DatabaseManager(config: config).fetchActiveThreads()
        }
        #expect(!FileManager.default.fileExists(atPath: config.stateDbUrl.path))
    }

    @Test func unsupportedSchemaIsReported() async throws {
        let fixture = try DatabaseFixture(schema: false)
        await #expect(throws: DatabaseReadError.incompatibleSchema) {
            try await DatabaseManager(config: fixture.config).fetchActiveThreads()
        }
    }

    @Test func malformedRowsAreReportedInsteadOfReturningPartialSuccess() async throws {
        let fixture = try DatabaseFixture()
        try fixture.thread("valid")
        try fixture.thread("bad")
        try fixture.execute("UPDATE orchestration_v2_projection_threads SET created_at='invalid' WHERE thread_id='bad'")
        await #expect(throws: DatabaseReadError.invalidData) {
            try await DatabaseManager(config: fixture.config).fetchActiveThreads()
        }
    }

    @Test func lockContentionIsReportedAsBusy() async throws {
        let fixture = try DatabaseFixture()
        try fixture.thread("t")
        try fixture.execute("BEGIN EXCLUSIVE")
        defer { try? fixture.execute("ROLLBACK") }
        await #expect(throws: DatabaseReadError.busy) {
            try await DatabaseManager(config: fixture.config).fetchActiveThreads()
        }
    }

    @Test func malformedJSONDuringSteppingIsNotAnEmptyInbox() async throws {
        let fixture = try DatabaseFixture()
        try fixture.thread("t", payload: "not-json")
        await #expect(throws: DatabaseReadError.invalidData) {
            try await DatabaseManager(config: fixture.config).fetchActiveThreads()
        }
    }

    @Test func persistedVisitsClearDoneAndLaterCompletionsRestoreIt() async throws {
        let fixture = try DatabaseFixture()
        try fixture.thread("t", payload: #"{"lastVisitedAt":"2026-11-04T00:00:00Z"}"#)
        try fixture.run("t", status: "completed", completedAt: "2026-11-04T00:00:02Z")
        let database = DatabaseManager(config: fixture.config)
        var threads = try await database.fetchActiveThreads()
        #expect(threads.first?.status == .done)

        // The menu bar reads the visit persisted by T3 Code on the next poll.
        try fixture.execute("UPDATE orchestration_v2_projection_threads SET payload_json=? WHERE thread_id='t'",
                            values: [#"{"lastVisitedAt":"2026-11-04T00:00:03Z"}"#])
        threads = try await database.fetchActiveThreads()
        #expect(threads.first?.status == .settled)

        try fixture.run("t", status: "completed", ordinal: 2, completedAt: "2026-11-04T00:00:04Z")
        threads = try await database.fetchActiveThreads()
        #expect(threads.first?.status == .done)
    }

    @Test(arguments: ["{}", #"{"lastVisitedAt":"invalid"}"#])
    func unknownVisitDoesNotMarkEveryCompletionUnread(_ payload: String) async throws {
        let fixture = try DatabaseFixture()
        try fixture.thread("t", payload: payload)
        try fixture.run("t", status: "completed", completedAt: "2026-11-04T00:00:02Z")
        let threads = try await DatabaseManager(config: fixture.config).fetchActiveThreads()
        #expect(threads.first?.status == .settled)
    }
}
