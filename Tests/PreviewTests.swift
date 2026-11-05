import Foundation
import Testing
@testable import T3CodeBar

struct PreviewTextTests {
    @Test func shortPlainTextIsKeptAsIs() {
        #expect(PreviewText.excerpt("All four commits are pushed.") == "All four commits are pushed.")
    }

    @Test func codeBlocksAreDroppedEvenWhenNeverClosed() {
        #expect(PreviewText.excerpt("Run this:\n```sh\nswift test\n```\nThen push.") == "Run this:\nThen push.")
        #expect(PreviewText.excerpt("Before\n```swift\nlet x = 1") == "Before")
        #expect(PreviewText.excerpt("```\nonly code\n```") == "")
    }

    @Test func markdownMarkersAreRemoved() {
        let source = "## Result\n**CI is green** and `swift test` passed, see [the run](https://example.com/run/1).\n- first\n* second\n> quoted"
        #expect(PreviewText.excerpt(source) == "Result\nCI is green and swift test passed, see the run.\n• first\n• second\nquoted")
    }

    @Test func longRepliesAreCutToFiveLinesWithAnEllipsis() {
        let source = (1...9).map { "line \($0)" }.joined(separator: "\n")
        #expect(PreviewText.excerpt(source) == "line 1\nline 2\nline 3\nline 4\nline 5…")
    }

    @Test func longLinesAreCutAtAWordBoundary() {
        let source = String(repeating: "word ", count: 200)
        let excerpt = PreviewText.excerpt(source)
        #expect(excerpt.count <= PreviewText.maxCharacters + 1)
        #expect(excerpt.hasSuffix("word…"))
    }
}

struct PreviewDatabaseTests {
    private func database(_ fixture: DatabaseFixture) -> DatabaseManager { DatabaseManager(config: fixture.config) }

    @Test func returnsTheLatestAssistantReply() async throws {
        let fixture = try DatabaseFixture()
        try fixture.message("t1", text: "old reply", createdAt: "2026-11-04T10:00:00Z")
        try fixture.message("t1", text: "newest reply", createdAt: "2026-11-04T12:00:00Z")
        try fixture.message("t1", text: "middle reply", createdAt: "2026-11-04T11:00:00Z")
        let preview = try await database(fixture).preview(threadId: "t1")
        #expect(preview?.text == "newest reply")
        #expect(preview?.updatedAt != nil)
    }

    @Test func ignoresUserMessagesAndOtherThreads() async throws {
        let fixture = try DatabaseFixture()
        try fixture.message("t1", role: "user", text: "a private question", createdAt: "2026-11-04T12:00:00Z")
        try fixture.message("t2", text: "another thread", createdAt: "2026-11-04T13:00:00Z")
        #expect(try await database(fixture).preview(threadId: "t1") == nil)
    }

    @Test func skipsRepliesWithNothingReadable() async throws {
        let fixture = try DatabaseFixture()
        try fixture.message("t1", text: "readable", createdAt: "2026-11-04T10:00:00Z")
        try fixture.message("t1", text: "", createdAt: "2026-11-04T11:00:00Z")
        try fixture.message("t1", text: "```\nonly code\n```", createdAt: "2026-11-04T12:00:00Z")
        // The newest reply is only code, so there is nothing to preview rather than a stale older one.
        #expect(try await database(fixture).preview(threadId: "t1") == nil)
    }

    @Test func hugeRepliesAreCappedInSQL() async throws {
        let fixture = try DatabaseFixture()
        try fixture.message("t1", text: String(repeating: "x ", count: 100_000), createdAt: "2026-11-04T10:00:00Z")
        let preview = try await database(fixture).preview(threadId: "t1")
        #expect((preview?.text.count ?? .max) <= PreviewText.maxCharacters + 1)
    }

    @Test func aMissingDatabaseIsAnErrorNotAnEmptyPreview() async throws {
        let fixture = try DatabaseFixture(schema: false)
        try FileManager.default.removeItem(at: fixture.config.stateDbUrl)
        await #expect(throws: DatabaseReadError.missing) { try await DatabaseManager(config: fixture.config).preview(threadId: "t1") }
    }
}

@MainActor
struct PreviewControllerTests {
    private func waitUntil(_ condition: () -> Bool) async {
        for _ in 0..<100 where !condition() { try? await Task.sleep(for: .milliseconds(10)) }
    }

    private func controller(delay: Duration = .milliseconds(40), counter: Counter) -> PreviewController {
        PreviewController(delay: delay) { id in
            counter.loads.append(id)
            return ThreadPreview(text: "reply for \(id)", updatedAt: nil)
        }
    }

    @MainActor final class Counter { var loads: [String] = [] }

    @Test func showsOnlyAfterTheDelayHasPassed() async {
        let counter = Counter()
        let controller = controller(delay: .milliseconds(150), counter: counter)
        controller.hover(true, threadId: "a")
        try? await Task.sleep(for: .milliseconds(50))
        #expect(controller.preview == nil)
        #expect(counter.loads.isEmpty)
        await waitUntil { controller.preview != nil }
        #expect(controller.preview?.text == "reply for a")
    }

    @Test func leavingBeforeTheDelayLoadsNothing() async {
        let counter = Counter()
        let controller = controller(delay: .milliseconds(80), counter: counter)
        controller.hover(true, threadId: "a")
        try? await Task.sleep(for: .milliseconds(20))
        controller.hover(false, threadId: "a")
        try? await Task.sleep(for: .milliseconds(150))
        #expect(counter.loads.isEmpty)
        #expect(controller.preview == nil)
    }

    @Test func leavingHidesAnOpenPreview() async {
        let counter = Counter()
        let controller = controller(counter: counter)
        controller.hover(true, threadId: "a")
        await waitUntil { controller.preview != nil }
        controller.hover(false, threadId: "a")
        #expect(controller.preview == nil)
    }

    @Test func movingToAnotherRowRestartsTheWait() async {
        let counter = Counter()
        let controller = controller(delay: .milliseconds(80), counter: counter)
        controller.hover(true, threadId: "a")
        try? await Task.sleep(for: .milliseconds(50))
        controller.hover(true, threadId: "b")
        await waitUntil { controller.preview != nil }
        #expect(counter.loads == ["b"])
    }

    @Test func theDefaultDelayIsHalfASecond() {
        #expect(PreviewController.defaultDelay == .milliseconds(500))
    }
}
