import SwiftUI

enum InquiryPreferences {
    static let loadCitedPortraits = "inquiry.loadCitedPortraits"
}

struct InquirySettingsView: View {
    @AppStorage(InquiryPreferences.loadCitedPortraits) private var loadCitedPortraits = true
    @AppStorage(InquiryPermissionPreferences.connectorMode)
    private var connectorMode = InquiryConnectorPermissionMode.askEveryTime.rawValue
    @State private var showYoloConfirmation = false

    var body: some View {
        TabView {
            Form {
                Section("Public connector permission") {
                    Picker("Network mode", selection: permissionBinding) {
                        ForEach(InquiryConnectorPermissionMode.allCases) { mode in
                            Text(mode.title).tag(mode)
                        }
                    }
                    .pickerStyle(.radioGroup)

                    Text(permissionExplanation)
                        .foregroundStyle(.secondary)

                    Label(
                        "YOLO mode applies only to low-risk public-query plans that the engine marks eligible. Sensitive context, package and flight identifiers, live aircraft tracking, account access, and unsupported destinations still fail closed.",
                        systemImage: "exclamationmark.shield.fill"
                    )
                    .foregroundStyle(.secondary)

                    Label(
                        "The Live workspace is manual, not a background feed. Its plan discloses the NASA EONET request and Apple map display before either loads; each refresh is a new bounded snapshot.",
                        systemImage: "globe.americas.fill"
                    )
                    .foregroundStyle(.secondary)
                }

                Section("Remote media") {
                    Toggle(
                        "Load cited portrait previews automatically",
                        isOn: $loadCitedPortraits
                    )
                    .accessibilityIdentifier("inquiry.settings.remotePortraits")

                    Text("Only identity-bound Wikimedia Commons portraits with accepted file-specific reuse terms are eligible for automatic loading. Rights-accepted event media remains off per result until you choose Load in Inquiry; all other remote images stay click-only.")
                        .foregroundStyle(.secondary)

                    Label(
                        "Loading a preview reveals your IP address, request time, and the cited image URL to Wikimedia. Inquiry uses an ephemeral session without cookies or stored credentials.",
                        systemImage: "hand.raised.fill"
                    )
                    .foregroundStyle(.secondary)

                    Label(
                        "Offline results never fetch portrait previews. Exported HTML reports also remain click-only.",
                        systemImage: "network.slash"
                    )
                    .foregroundStyle(.secondary)
                }
            }
            .formStyle(.grouped)
            .tabItem {
                Label("Privacy", systemImage: "lock.shield")
            }
        }
        .frame(width: 560, height: 570)
        .scenePadding()
        .alert("Enable YOLO mode?", isPresented: $showYoloConfirmation) {
            Button("Cancel", role: .cancel) {}
            Button("Enable automatic public requests", role: .destructive) {
                connectorMode = InquiryConnectorPermissionMode.automaticPublicWeb.rawValue
            }
        } message: {
            Text("Eligible public research queries will be sent automatically to the exact services shown in each local execution plan. Opening a Live snapshot may also load its in-app Apple map without another prompt. These requests can reveal your IP address, request time, public query terms, or map viewport. You can return to Ask every time at any time.")
        }
    }

    private var permissionBinding: Binding<InquiryConnectorPermissionMode> {
        Binding(
            get: {
                InquiryConnectorPermissionMode(rawValue: connectorMode) ?? .askEveryTime
            },
            set: { mode in
                if mode == .automaticPublicWeb,
                   connectorMode != InquiryConnectorPermissionMode.automaticPublicWeb.rawValue {
                    showYoloConfirmation = true
                } else {
                    connectorMode = mode.rawValue
                }
            }
        )
    }

    private var permissionExplanation: String {
        switch InquiryConnectorPermissionMode(rawValue: connectorMode) ?? .askEveryTime {
        case .askEveryTime:
            "Inquiry shows the exact services, destinations, and outbound fields before every public-web run. Approval is valid only for that plan fingerprint."
        case .automaticPublicWeb:
            "Eligible low-risk public queries run automatically. The connector audit remains attached to every result."
        case .offlineOnly:
            "Inquiry uses only local calculations, imported data, reference tables, and curated offline capability notes."
        }
    }
}
