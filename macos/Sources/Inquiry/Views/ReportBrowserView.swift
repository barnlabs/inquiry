import AppKit
import SwiftUI
import WebKit

@MainActor
final class ReportBrowserController: ObservableObject {
    weak var webView: WKWebView?

    func printReport() {
        guard let webView else { return }
        let operation = webView.printOperation(with: NSPrintInfo.shared)
        operation.showsPrintPanel = true
        operation.showsProgressPanel = true
        operation.run()
    }
}

struct ReportBrowserView: View {
    let url: URL
    let close: () -> Void
    @StateObject private var controller = ReportBrowserController()

    var body: some View {
        ReportWebView(url: url, controller: controller)
            .frame(minWidth: 720, minHeight: 560)
            .toolbar {
                ToolbarItemGroup(placement: .primaryAction) {
                    Button("Print", systemImage: "printer", action: controller.printReport)
                        .keyboardShortcut("p", modifiers: [.command])
                    Button("Close", systemImage: "xmark", action: close)
                        .keyboardShortcut(.cancelAction)
                }
            }
            .accessibilityIdentifier("inquiry.report.browser")
    }
}

private struct ReportWebView: NSViewRepresentable {
    let url: URL
    let controller: ReportBrowserController

    func makeCoordinator() -> Coordinator {
        Coordinator(initialFileURL: url)
    }

    func makeNSView(context: Context) -> WKWebView {
        let configuration = WKWebViewConfiguration()
        configuration.websiteDataStore = .nonPersistent()
        configuration.defaultWebpagePreferences.allowsContentJavaScript = true
        let webView = WKWebView(frame: .zero, configuration: configuration)
        webView.navigationDelegate = context.coordinator
        controller.webView = webView
        webView.loadFileURL(url, allowingReadAccessTo: url.deletingLastPathComponent())
        return webView
    }

    func updateNSView(_ webView: WKWebView, context: Context) {
        context.coordinator.initialFileURL = url.standardizedFileURL
        if webView.url?.standardizedFileURL != url.standardizedFileURL {
            webView.loadFileURL(url, allowingReadAccessTo: url.deletingLastPathComponent())
        }
        controller.webView = webView
    }

    @MainActor
    final class Coordinator: NSObject, WKNavigationDelegate {
        var initialFileURL: URL

        init(initialFileURL: URL) {
            self.initialFileURL = initialFileURL.standardizedFileURL
        }

        func webView(
            _: WKWebView,
            decidePolicyFor navigationAction: WKNavigationAction,
            decisionHandler: @escaping @MainActor @Sendable (WKNavigationActionPolicy) -> Void
        ) {
            guard let target = navigationAction.request.url else {
                decisionHandler(.cancel)
                return
            }
            if target.standardizedFileURL == initialFileURL || target.scheme == "about" {
                decisionHandler(.allow)
                return
            }
            if navigationAction.navigationType == .linkActivated,
               let scheme = target.scheme?.lowercased(),
               ["https", "http"].contains(scheme),
               target.user == nil,
               target.password == nil {
                NSWorkspace.shared.open(target)
            }
            decisionHandler(.cancel)
        }
    }
}
