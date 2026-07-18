import AppKit
import SwiftUI

final class AppDelegate: NSObject, NSApplicationDelegate {
    func applicationDidFinishLaunching(_ notification: Notification) {
        NSApp.setActivationPolicy(.regular)
        NSApp.activate(ignoringOtherApps: true)
    }
}

@main
struct InquiryApp: App {
    @NSApplicationDelegateAdaptor(AppDelegate.self) private var appDelegate
    @StateObject private var store = ResearchStore()

    var body: some Scene {
        WindowGroup("Inquiry", id: "main") {
            ContentView(store: store)
                .frame(minWidth: 760, minHeight: 560)
        }
        .commands {
            CommandGroup(replacing: .newItem) {
                Button("New Inquiry") { store.startNewInquiry() }
                    .keyboardShortcut("n", modifiers: [.command])
            }
            CommandGroup(after: .newItem) {
                Button("Start Research") { store.focusRequestID = UUID() }
                    .keyboardShortcut("l", modifiers: [.command])
                Button("Open Interactive Report") { store.openInteractiveReport() }
                    .keyboardShortcut("o", modifiers: [.command, .shift])
                    .disabled(store.report == nil || store.isRunning || store.isRenderingReport)
            }
        }

        Settings {
            InquirySettingsView()
        }
    }
}
