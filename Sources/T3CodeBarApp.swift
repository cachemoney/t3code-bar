import SwiftUI
import AppKit

final class AppDelegate: NSObject, NSApplicationDelegate {
    func applicationDidFinishLaunching(_ notification: Notification) {
        // Run purely as an accessory (menu bar item only, no Dock icon)
        NSApp.setActivationPolicy(.accessory)

        // Monitor right-click on the MenuBarExtra status item
        NSEvent.addLocalMonitorForEvents(matching: [.rightMouseDown, .rightMouseUp]) { event in
            guard let window = event.window,
                  (window.level == .statusBar || NSStringFromClass(type(of: window)).localizedCaseInsensitiveContains("statusbar")) else {
                return event
            }

            if event.type == .rightMouseDown {
                let menu = NSMenu()
                let quitItem = NSMenuItem(
                    title: "Quit T3 Code Bar",
                    action: #selector(NSApplication.terminate(_:)),
                    keyEquivalent: "q"
                )
                quitItem.target = NSApp
                menu.addItem(quitItem)

                let location = window.contentView?.convert(event.locationInWindow, from: nil) ?? event.locationInWindow
                menu.popUp(positioning: quitItem, at: location, in: window.contentView)
            }
            return nil
        }
    }
}

@main
struct T3CodeBarApp: App {
    @NSApplicationDelegateAdaptor(AppDelegate.self) var appDelegate
    @StateObject private var monitor = ThreadMonitor()

    var body: some Scene {
        MenuBarExtra {
            PopoverView(monitor: monitor)
        } label: {
            MenuBarIconView(
                status: monitor.overallStatus,
                activeCount: monitor.runningCount > 0 ? monitor.runningCount : monitor.attentionCount
            )
            .contextMenu {
                Button("Quit T3 Code Bar") {
                    NSApplication.shared.terminate(nil)
                }
            }
        }
        .menuBarExtraStyle(.window)
    }
}
