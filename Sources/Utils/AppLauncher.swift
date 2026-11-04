import Foundation
import AppKit

@MainActor
public enum AppLauncher {
    public static func activateT3Code() {
        if let app = NSRunningApplication.runningApplications(withBundleIdentifier: "com.t3tools.t3code").first {
            app.activate()
            return
        }
        let appURL = NSWorkspace.shared.urlForApplication(withBundleIdentifier: "com.t3tools.t3code")
            ?? ["T3 Code.app", "T3 Code (Nightly).app"].map {
                URL(fileURLWithPath: "/Applications/\($0)")
            }.first { FileManager.default.fileExists(atPath: $0.path) }
        guard let appURL else {
            showError("T3 Code is not installed or could not be located in Applications.")
            return
        }
        let configuration = NSWorkspace.OpenConfiguration()
        configuration.activates = true
        NSWorkspace.shared.openApplication(at: appURL, configuration: configuration) { _, error in
            if error != nil {
                Task { @MainActor in showError("T3 Code could not be opened.") }
            }
        }
    }

    public static func openThread(threadId: String) {
        openThreadURL(threadId: threadId, browser: false)
    }

    public static func openThread(thread: ThreadItem) {
        openThread(threadId: thread.id)
    }

    public static func openNewThread() {
        guard let url = URL(string: "t3code://new") else { return }
        if !NSWorkspace.shared.open(url) { showError("The T3 Code URL handler could not be opened.") }
    }

    public static func openThreadInBrowser(threadId: String) {
        openThreadURL(threadId: threadId, browser: true)
    }

    private static func openThreadURL(threadId: String, browser: Bool) {
        do {
            let url = try EnvironmentConfig.shared.threadURL(threadId: threadId, browser: browser)
            if !NSWorkspace.shared.open(url) { showError("The thread link could not be opened.") }
        } catch {
            showError(error.localizedDescription)
        }
    }

    public static func copyThreadLink(threadId: String) {
        do {
            let url = try EnvironmentConfig.shared.threadURL(threadId: threadId)
            NSPasteboard.general.clearContents()
            NSPasteboard.general.setString(url.absoluteString, forType: .string)
        } catch {
            showError(error.localizedDescription)
        }
    }

    private static func showError(_ message: String) {
        let alert = NSAlert()
        alert.messageText = "T3 Code Bar"
        alert.informativeText = message
        alert.alertStyle = .warning
        alert.runModal()
    }
}
