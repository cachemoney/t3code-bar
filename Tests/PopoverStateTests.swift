import Foundation
import Testing
@testable import T3CodeBar

struct RowAccessibilityTests {
    @Test func rowReadsAsOneSentenceWithTheStatusInWords() {
        let label = ThreadRowView.accessibilityLabel(project: "api", title: "Fix login", branch: "fix/login", status: .approval)
        #expect(label == "Fix login, in api, on branch fix/login, Approval")
    }

    @Test func idleThreadsAndMissingBranchesAddNothing() {
        #expect(ThreadRowView.accessibilityLabel(project: "api", title: "Fix login", branch: nil, status: .settled) == "Fix login, in api")
        #expect(ThreadRowView.accessibilityLabel(project: "api", title: "Fix login", branch: "", status: .settled) == "Fix login, in api")
    }

    @Test func everyStatusThatIsNotIdleIsAnnounced() {
        let statuses: [ThreadStatus] = [.working(startedAt: nil), .goal(startedAt: nil), .approval, .input,
                                        .request, .woke, .failed, .waiting, .limited, .done]
        for status in statuses {
            let label = ThreadRowView.accessibilityLabel(project: "p", title: "t", branch: nil, status: status)
            #expect(label.hasSuffix(status.label))
        }
    }
}

struct StatusIconTests {
    @Test @MainActor func everyIconShipsAndLoads() {
        for icon in StatusIcon.allCases {
            #expect(icon.image != nil, "missing \(icon.resourceName).svg")
        }
    }

    @Test func onlyIdleThreadsHaveNoIcon() {
        let all: [ThreadStatus] = [.working(startedAt: nil), .goal(startedAt: nil), .approval, .input,
                                   .request, .woke, .failed, .waiting, .limited, .done]
        for status in all { #expect(status.icon != nil, "\(status.label) has no icon") }
        #expect(ThreadStatus.settled.icon == nil)
    }
}
