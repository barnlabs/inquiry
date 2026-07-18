import MapKit
import SwiftUI

struct ContentView: View {
  @ObservedObject var store: ResearchStore
  @FocusState private var queryFocused: Bool
  @State private var originalSendAcknowledged = false
  @State private var workspace: InquiryWorkspace = .research
  @AppStorage(InquiryPreferences.loadCitedPortraits) private var loadCitedPortraits = true

  var body: some View {
    NavigationSplitView {
      List {
        Section("Workspace") {
          Button {
            workspace = .research
          } label: {
            Label("Research", systemImage: "sparkle.magnifyingglass")
              .fontWeight(.semibold)
          }
          .buttonStyle(.plain)
          .accessibilityAddTraits(workspace == .research ? .isSelected : [])
          .accessibilityIdentifier("inquiry.workspace.research")
          Button {
            workspace = .study
          } label: {
            Label("InquiryStudy", systemImage: "books.vertical.fill")
              .fontWeight(.semibold)
          }
          .buttonStyle(.plain)
          .accessibilityAddTraits(workspace == .study ? .isSelected : [])
          .accessibilityIdentifier("inquiry.workspace.study")
          Button {
            workspace = .live
          } label: {
            Label("Live", systemImage: "globe.americas.fill")
              .fontWeight(.semibold)
          }
          .buttonStyle(.plain)
          .accessibilityAddTraits(workspace == .live ? .isSelected : [])
          .accessibilityIdentifier("inquiry.workspace.live")
        }
        if workspace == .research, !store.recentQueries.isEmpty {
          Section("Recent") {
            ForEach(store.recentQueries, id: \.self) { query in
              Button {
                store.selectRecent(query)
              } label: {
                Label {
                  Text(query).lineLimit(2)
                } icon: {
                  Image(systemName: "clock")
                }
              }
              .buttonStyle(.plain)
            }
          }
        }
      }
      .listStyle(.sidebar)
      .navigationSplitViewColumnWidth(min: 210, ideal: 250, max: 320)
    } detail: {
      VStack(spacing: 0) {
        if let errorMessage = store.errorMessage {
          ErrorBanner(message: errorMessage) {
            store.errorMessage = nil
          }
          Divider()
        }
        if workspace == .research {
          VStack(spacing: 0) {
            ComposerView(
              store: store,
              queryFocused: $queryFocused
            )
            Divider()
            Group {
              if store.isRunning {
                ResearchProgressView(
                  offline: store.offline,
                  cancel: store.cancelResearch
                )
              } else if let report = store.report {
                ReportView(
                  report: report,
                  loadCitedPortraits: loadCitedPortraits,
                  isRenderingReport: store.isRenderingReport,
                  openInteractiveReport: store.openInteractiveReport,
                  exportWord: store.exportReportForWord,
                  exportExcel: store.exportReportForExcel
                )
              } else {
                EmptyResearchView()
              }
            }
          }
        } else if workspace == .study {
          InquiryStudyView(store: store)
        } else {
          LiveWorkspaceView(store: store)
        }
      }
      .background(.background)
      .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
    }
    .onChange(of: store.focusRequestID) { _, _ in queryFocused = true }
    .sheet(
      isPresented: Binding(
        get: { store.privacyAssessment != nil },
        set: {
          if !$0 {
            store.privacyAssessment = nil
            originalSendAcknowledged = false
          }
        }
      )
    ) {
      if let assessment = store.privacyAssessment {
        SensitiveQueryReview(
          assessment: assessment,
          originalQuery: store.query,
          originalSendAcknowledged: $originalSendAcknowledged,
          useOffline: store.useOfflineForSensitiveQuery,
          sendRedacted: store.sendRedactedSensitiveQuery,
          sendOriginal: store.confirmSensitiveWebQuery,
          cancel: { store.privacyAssessment = nil }
        )
      }
    }
    .onChange(of: store.privacyAssessment?.redactedQuery) { _, _ in
      if shouldResetOriginalSendAcknowledgement() {
        originalSendAcknowledged = false
      }
    }
    .sheet(
      item: Binding(
        get: { store.pendingExecutionPlan },
        set: { if $0 == nil { store.dismissPendingExecutionPlan() } }
      )
    ) { plan in
      ConnectorPermissionReview(
        plan: plan,
        approve: store.approvePendingExecutionPlan,
        keepOffline: store.runPendingExecutionOffline,
        cancel: {
          store.dismissPendingExecutionPlan()
          store.focusRequestID = UUID()
        }
      )
    }
    .sheet(
      item: Binding(
        get: { store.pendingLiveExecutionPlan },
        set: { if $0 == nil { store.dismissPendingLiveExecutionPlan() } }
      )
    ) { plan in
      LivePermissionReview(
        plan: plan,
        approve: store.approvePendingLiveExecutionPlan,
        cancel: store.dismissPendingLiveExecutionPlan
      )
    }
    .sheet(
      isPresented: Binding(
        get: { store.interactiveReportURL != nil },
        set: { if !$0 { store.closeInteractiveReport() } }
      )
    ) {
      if let url = store.interactiveReportURL {
        ReportBrowserView(url: url, close: store.closeInteractiveReport)
      }
    }
    .toolbar {
      ToolbarItem(placement: .primaryAction) {
        Button("New Inquiry", systemImage: "square.and.pencil") {
          store.startNewInquiry()
        }
        .help("Start a new inquiry (Command-N)")
      }
    }
  }
}

private enum InquiryWorkspace {
  case research
  case study
  case live
}

private struct LivePermissionReview: View {
  let plan: InquiryExecutionPlan
  let approve: () -> Void
  let cancel: () -> Void

  var body: some View {
    VStack(alignment: .leading, spacing: 18) {
      Label("Load one Live snapshot", systemImage: "hand.raised.fill")
        .font(.title2.bold())
      Text(
        "Nothing has been requested yet. Approval applies to this one snapshot and the displayed map; Inquiry does not poll in the background."
      )
      .foregroundStyle(.secondary)
      GroupBox("Services that may receive data") {
        VStack(alignment: .leading, spacing: 14) {
          ForEach(plan.connectors) { connector in
            VStack(alignment: .leading, spacing: 4) {
              Text(connector.service).fontWeight(.semibold)
              Text(connector.destinations.joined(separator: ", "))
                .font(.caption.monospaced())
              Text(connector.outboundData).font(.callout)
              Text(connector.purpose).font(.caption).foregroundStyle(.secondary)
            }
            if connector.id != plan.connectors.last?.id { Divider() }
          }
        }
        .padding(.top, 4)
      }
      GroupBox("Native map display") {
        VStack(alignment: .leading, spacing: 4) {
          Text("Apple MapKit").fontWeight(.semibold)
          Text("System-managed Apple Maps endpoints")
            .font(.caption.monospaced())
          Text(
            "The displayed world viewport, your IP address, device/network metadata, and later pan or zoom requests may be visible to Apple."
          )
          .font(.callout)
          Text(
            "Inquiry does not deliberately submit an EONET event title, source link, search filter, account credential, or precise device location to MapKit. The map is created only after this approval; use Always offline to keep it off."
          )
          .font(.caption)
          .foregroundStyle(.secondary)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(.top, 4)
      }
      Text(plan.disclosure).font(.caption).foregroundStyle(.secondary)
      HStack {
        Button("Cancel", role: .cancel, action: cancel)
          .keyboardShortcut(.cancelAction)
        Spacer()
        Button("Approve once", action: approve)
          .buttonStyle(.borderedProminent)
          .keyboardShortcut(.defaultAction)
      }
    }
    .padding(24)
    .frame(minWidth: 620, idealWidth: 720, minHeight: 420)
    .accessibilityIdentifier("inquiry.live.permissionReview")
  }
}

private struct ErrorBanner: View {
  let message: String
  let dismiss: () -> Void

  var body: some View {
    HStack(alignment: .top, spacing: 10) {
      Image(systemName: "exclamationmark.triangle.fill")
        .foregroundStyle(.orange)
      VStack(alignment: .leading, spacing: 2) {
        Text("Action could not be completed")
          .fontWeight(.semibold)
        Text(message)
          .font(.caption)
          .foregroundStyle(.secondary)
          .textSelection(.enabled)
      }
      Spacer(minLength: 12)
      Button("Dismiss", systemImage: "xmark", action: dismiss)
        .labelStyle(.iconOnly)
        .buttonStyle(.plain)
    }
    .padding(.horizontal, 18)
    .padding(.vertical, 10)
    .background(.orange.opacity(0.09))
    .accessibilityElement(children: .contain)
    .accessibilityIdentifier("inquiry.error.banner")
  }
}

private struct ResearchProgressView: View {
  let offline: Bool
  let cancel: () -> Void

  var body: some View {
    VStack(spacing: 14) {
      ProgressView()
        .controlSize(.large)
      Text(offline ? "Researching local capabilities…" : "Researching public sources…")
        .font(.headline)
      Text(
        offline
          ? "No public connector or remote portrait request will be made."
          : "Accepted evidence will appear with its source and connector record."
      )
      .font(.callout)
      .foregroundStyle(.secondary)
      .multilineTextAlignment(.center)
      Button("Cancel research", role: .cancel, action: cancel)
        .keyboardShortcut(.cancelAction)
    }
    .frame(maxWidth: .infinity, maxHeight: .infinity)
    .padding(32)
    .accessibilityIdentifier(offline ? "inquiry.loading.offline" : "inquiry.loading.live")
  }
}

private struct ComposerView: View {
  @ObservedObject var store: ResearchStore
  var queryFocused: FocusState<Bool>.Binding
  @AppStorage(InquiryPermissionPreferences.connectorMode)
  private var connectorMode = InquiryConnectorPermissionMode.askEveryTime.rawValue

  var body: some View {
    VStack(alignment: .leading, spacing: 12) {
      ViewThatFits(in: .horizontal) {
        HStack(alignment: .center, spacing: 18) {
          brand
          Spacer(minLength: 18)
          permissionStatus
        }
        VStack(alignment: .leading, spacing: 10) {
          brand
          permissionStatus
        }
      }
      HStack(spacing: 8) {
        Label(
          permissionMode == .offlineOnly
            ? "Always offline"
            : permissionMode == .automaticPublicWeb
              ? "YOLO mode · eligible public plans run automatically"
              : "Permission required before public requests",
          systemImage: permissionMode == .offlineOnly
            ? "network.slash"
            : permissionMode == .automaticPublicWeb
              ? "bolt.shield"
              : "checkmark.shield"
        )
        .font(.caption)
        .foregroundStyle(.secondary)
        .accessibilityIdentifier("inquiry.network.status")
        Spacer()
        SettingsLink {
          Label("Permissions & media", systemImage: "gearshape")
        }
        .font(.caption)
        .buttonStyle(.plain)
      }
      ViewThatFits(in: .horizontal) {
        HStack(alignment: .bottom, spacing: 10) {
          queryField
          actionButtons
        }
        VStack(alignment: .leading, spacing: 10) {
          queryField
          actionButtons
        }
      }
    }
    .padding(.horizontal, 20)
    .padding(.vertical, 16)
  }

  private var brand: some View {
    VStack(alignment: .leading, spacing: 2) {
      Text("BarnLabs Inquiry")
        .font(.title2.bold())
      Text("Ask broadly. Verify deeply.")
        .font(.callout)
        .foregroundStyle(.secondary)
    }
  }

  private var permissionStatus: some View {
    Label(permissionMode.title, systemImage: permissionMode.systemImage)
      .font(.caption)
      .foregroundStyle(.secondary)
      .help(permissionMode.help)
      .accessibilityIdentifier("inquiry.network.permissionMode")
  }

  private var permissionMode: InquiryConnectorPermissionMode {
    InquiryConnectorPermissionMode(rawValue: connectorMode) ?? .askEveryTime
  }

  private var queryField: some View {
    TextField(
      "Research a public subject, calculation, source, or course concept…",
      text: $store.query,
      axis: .vertical
    )
    .textFieldStyle(.roundedBorder)
    .lineLimit(1...4)
    .focused(queryFocused)
    .onSubmit(store.research)
    .accessibilityLabel("Research question")
    .accessibilityIdentifier("inquiry.query")
  }

  private var actionButtons: some View {
    HStack(spacing: 8) {
      Button(action: store.research) {
        Label("Research", systemImage: "arrow.right.circle.fill")
      }
      .buttonStyle(.borderedProminent)
      .keyboardShortcut(.return, modifiers: [.command])
      .disabled(
        store.query.trimmingCharacters(in: .whitespacesAndNewlines).count < 3
          || store.isRunning
      )
      .accessibilityIdentifier("inquiry.research.start")
      if store.isRunning {
        Button("Cancel", role: .cancel) { store.cancelResearch() }
          .keyboardShortcut(.cancelAction)
      }
    }
  }
}

extension InquiryConnectorPermissionMode {
  fileprivate var systemImage: String {
    switch self {
    case .askEveryTime: "hand.raised.fill"
    case .automaticPublicWeb: "bolt.shield.fill"
    case .offlineOnly: "network.slash"
    }
  }

  fileprivate var help: String {
    switch self {
    case .askEveryTime:
      "Inquiry shows the exact connector plan before any public request."
    case .automaticPublicWeb:
      "Only low-risk plans explicitly marked eligible run automatically."
    case .offlineOnly:
      "No public connector or remote media request is allowed."
    }
  }
}

private struct ConnectorPermissionReview: View {
  let plan: InquiryExecutionPlan
  let approve: () -> Void
  let keepOffline: () -> Void
  let cancel: () -> Void

  var body: some View {
    VStack(alignment: .leading, spacing: 18) {
      Label(
        plan.intent.clarification == nil ? "Review public requests" : "Inquiry needs more detail",
        systemImage: plan.intent.clarification == nil
          ? "hand.raised.fill" : "questionmark.bubble.fill"
      )
      .font(.title2.bold())

      Text(plan.intent.clarification ?? plan.intent.rationale)
        .foregroundStyle(.secondary)

      LabeledContent("Intent", value: plan.intent.label)
      GroupBox("Local query preview") {
        Text(plan.queryPreview)
          .textSelection(.enabled)
          .frame(maxWidth: .infinity, alignment: .leading)
          .padding(.top, 3)
      }

      if !plan.connectors.isEmpty {
        GroupBox("Services that will receive data") {
          VStack(alignment: .leading, spacing: 13) {
            ForEach(plan.connectors) { connector in
              VStack(alignment: .leading, spacing: 4) {
                HStack {
                  Text(connector.service).fontWeight(.semibold)
                  Spacer()
                  Text(connector.destinations.joined(separator: ", "))
                    .font(.caption.monospaced())
                    .foregroundStyle(.secondary)
                }
                Text(connector.outboundData)
                  .font(.callout)
                Text(connector.purpose)
                  .font(.caption)
                  .foregroundStyle(.secondary)
              }
              if connector.id != plan.connectors.last?.id { Divider() }
            }
          }
          .padding(.top, 4)
        }
        Text(plan.disclosure)
          .font(.caption)
          .foregroundStyle(.secondary)
      }

      HStack {
        Button(
          plan.intent.clarification == nil ? "Cancel" : "Edit question", role: .cancel,
          action: cancel
        )
        .keyboardShortcut(.cancelAction)
        Spacer()
        if !plan.connectors.isEmpty {
          Button("Keep this run offline", action: keepOffline)
          Button("Approve once", action: approve)
            .buttonStyle(.borderedProminent)
            .keyboardShortcut(.defaultAction)
        }
      }
    }
    .padding(24)
    .frame(minWidth: 620, idealWidth: 720, minHeight: 420)
    .accessibilityIdentifier("inquiry.connector.permissionReview")
  }
}

private struct EmptyResearchView: View {
  var body: some View {
    ContentUnavailableView {
      Label(
        "A research workbench, not a chatbot", systemImage: "point.3.connected.trianglepath.dotted")
    } description: {
      Text(
        "Inquiry gathers public evidence, keeps sources and data periods attached, performs deterministic calculations, and builds an interactive dossier you can audit."
      )
    } actions: {
      Text("Try: “Compare GDP, population, safety sources, and public-health metrics for Kenya.”")
        .font(.callout).foregroundStyle(.secondary).textSelection(.enabled)
    }
  }
}

private struct InquiryStudyView: View {
  @ObservedObject var store: ResearchStore
  @State private var confirmDelete = false

  var body: some View {
    ScrollView {
      VStack(alignment: .leading, spacing: 20) {
        VStack(alignment: .leading, spacing: 7) {
          Label("InquiryStudy", systemImage: "books.vertical.fill")
            .font(.largeTitle.bold())
          Text(
            "Search your professor’s presentations and your own notes without sending their contents to Inquiry’s web connectors."
          )
          .foregroundStyle(.secondary)
          Label(
            "The Rust indexer initiates no network request. macOS or a configured iCloud, OneDrive, or other File Provider may still hydrate cloud-only files or sync an export folder.",
            systemImage: "externaldrive.badge.icloud"
          )
          .font(.caption)
          .foregroundStyle(.orange)
        }

        GroupBox("1. Choose one course folder") {
          VStack(alignment: .leading, spacing: 12) {
            HStack {
              Button("Choose course folder…", action: store.chooseStudyDirectory)
                .buttonStyle(.borderedProminent)
              if let directory = store.studyDirectory {
                Label(directory.lastPathComponent, systemImage: "folder.fill")
                  .fontWeight(.semibold)
              }
            }
            if let directory = store.studyDirectory {
              DisclosureGroup("Local path audit") {
                Text(directory.path)
                  .font(.caption.monospaced())
                  .textSelection(.enabled)
                  .frame(maxWidth: .infinity, alignment: .leading)
                  .padding(.top, 5)
              }
            }
            HStack {
              TextField("Course (optional)", text: $store.studyCourse)
              TextField("Instructor (optional)", text: $store.studyInstructor)
            }
            Toggle(
              "Include PowerPoint speaker notes",
              isOn: $store.includeSpeakerNotes
            )
            .help("Off by default because notes may contain hidden or unpublished material")
            Toggle(
              "I am authorized to process these materials and will review exports before sharing them.",
              isOn: $store.studyMaterialAuthorized
            )
          }
          .padding(.top, 6)
        }

        HStack {
          Button(action: store.buildStudyIndex) {
            Label(
              store.studyIndexSummary == nil ? "Build private index" : "Rebuild private index",
              systemImage: "lock.doc"
            )
          }
          .buttonStyle(.borderedProminent)
          .disabled(
            store.studyDirectory == nil
              || !store.studyMaterialAuthorized
              || store.studyIsRunning
          )
          if store.studyIsRunning {
            ProgressView()
            Button("Cancel", role: .cancel, action: store.cancelStudy)
          }
        }

        if let summary = store.studyIndexSummary {
          GroupBox("2. Private index") {
            VStack(alignment: .leading, spacing: 12) {
              HStack(spacing: 18) {
                studyMetric("Documents", value: summary.documentsIndexed)
                studyMetric("Segments", value: summary.segmentsIndexed)
                studyMetric("Skipped", value: summary.filesSkipped)
                studyMetric("App network requests", value: summary.applicationNetworkRequests)
              }
              Text(summary.notice)
                .font(.caption)
                .foregroundStyle(.secondary)
              ForEach(summary.warnings, id: \.self) { warning in
                Label(warning, systemImage: "exclamationmark.triangle")
                  .font(.caption)
                  .foregroundStyle(.secondary)
              }
              if !summary.skipped.isEmpty {
                DisclosureGroup("Skipped-file ledger") {
                  VStack(alignment: .leading, spacing: 6) {
                    ForEach(summary.skipped) { skipped in
                      Text("\(skipped.relativePath) — \(skipped.reason)")
                        .font(.caption.monospaced())
                        .textSelection(.enabled)
                    }
                  }
                  .frame(maxWidth: .infinity, alignment: .leading)
                  .padding(.top, 5)
                }
              }
              HStack {
                Button("Reveal private index", action: store.revealStudyIndex)
                Button("Delete private index", role: .destructive) {
                  confirmDelete = true
                }
              }
            }
            .padding(.top, 6)
          }

          GroupBox("3. Search exact course excerpts") {
            VStack(alignment: .leading, spacing: 12) {
              HStack {
                TextField(
                  "Use course vocabulary or an exact concept…",
                  text: $store.studyQuery
                )
                .onSubmit(store.searchStudyIndex)
                Button("Search notes", action: store.searchStudyIndex)
                  .buttonStyle(.borderedProminent)
                  .disabled(
                    store.studyQuery
                      .trimmingCharacters(in: .whitespacesAndNewlines)
                      .count < 2
                      || store.studyIsRunning
                  )
              }
              Text(
                "A result is a normalized quoted index excerpt, not an independently verified fact. Checksums detect changes but do not authenticate a separately editable index. Embedded instructions are always treated as untrusted text."
              )
              .font(.caption)
              .foregroundStyle(.secondary)
            }
            .padding(.top, 6)
          }
        }

        if let search = store.studySearch {
          VStack(alignment: .leading, spacing: 14) {
            HStack {
              Text("Cited matches").font(.title2.bold())
              Spacer()
              if !search.results.isEmpty {
                Button("Export recall pack…", action: store.exportStudyPack)
                  .buttonStyle(.borderedProminent)
                  .disabled(
                    store.studyIsRunning
                      || !search.results.contains(where: { $0.risks.isEmpty })
                  )
              }
            }
            if search.results.isEmpty {
              ContentUnavailableView(
                "No cited match",
                systemImage: "text.magnifyingglass",
                description: Text(
                  "Try the terminology used in the course material. Inquiry will not invent a card when no source span matches."
                )
              )
            }
            ForEach(search.results) { result in
              VStack(alignment: .leading, spacing: 9) {
                HStack {
                  Text("\(result.relativePath) — \(result.locator)")
                    .font(.headline)
                  Spacer()
                  Text(String(format: "%.3f", result.score))
                    .font(.caption.monospaced())
                    .foregroundStyle(.secondary)
                }
                Text(result.excerpt)
                  .textSelection(.enabled)
                if !result.risks.isEmpty {
                  Label(
                    "Recall export blocked: \(result.risks.map { $0.replacingOccurrences(of: "_", with: " ") }.joined(separator: ", "))",
                    systemImage: "exclamationmark.shield.fill"
                  )
                  .font(.caption)
                  .foregroundStyle(.orange)
                }
                Text("Normalized excerpt SHA-256 checksum \(result.contentHash)")
                  .font(.caption2.monospaced())
                  .foregroundStyle(.tertiary)
                  .textSelection(.enabled)
                Text("Original document SHA-256 checksum \(result.documentHash)")
                  .font(.caption2.monospaced())
                  .foregroundStyle(.tertiary)
                  .textSelection(.enabled)
              }
              .frame(maxWidth: .infinity, alignment: .leading)
              .padding(16)
              .background(.regularMaterial, in: RoundedRectangle(cornerRadius: 14))
            }
            ForEach(search.warnings, id: \.self) { warning in
              Text("Notice: \(warning)")
                .font(.caption)
                .foregroundStyle(.secondary)
            }
            if let message = store.studyExportMessage {
              Label(message, systemImage: "checkmark.circle.fill")
                .foregroundStyle(.green)
            }
          }
        }
      }
      .padding(28)
    }
    .confirmationDialog(
      "Delete this private study index?",
      isPresented: $confirmDelete,
      titleVisibility: .visible
    ) {
      Button("Delete index", role: .destructive, action: store.deleteStudyIndex)
      Button("Cancel", role: .cancel) {}
    } message: {
      Text(
        "The original course files are not changed. Search excerpts stored in the index will be removed."
      )
    }
  }

  private func studyMetric(_ label: String, value: Int) -> some View {
    VStack(alignment: .leading, spacing: 2) {
      Text(value.formatted()).font(.title3.bold())
      Text(label).font(.caption).foregroundStyle(.secondary)
    }
  }
}

private struct LiveWorkspaceView: View {
  @ObservedObject var store: ResearchStore
  @State private var search = ""

  var body: some View {
    VStack(spacing: 0) {
      HStack(alignment: .center, spacing: 14) {
        VStack(alignment: .leading, spacing: 3) {
          Text("Live")
            .font(.title2.bold())
          Text("A manually refreshed snapshot of NASA-curated natural-event records.")
            .font(.callout)
            .foregroundStyle(.secondary)
        }
        Spacer()
        Button(action: store.refreshLiveEvents) {
          if store.liveIsRunning {
            Label("Loading…", systemImage: "arrow.triangle.2.circlepath")
          } else {
            Label(
              store.liveSnapshot == nil ? "Load snapshot" : "Refresh once",
              systemImage: "arrow.clockwise")
          }
        }
        .buttonStyle(.borderedProminent)
        .disabled(store.liveIsRunning)
        .accessibilityIdentifier("inquiry.live.refresh")
        if store.liveIsRunning {
          Button("Cancel", role: .cancel, action: store.cancelLiveEvents)
        }
      }
      .padding(.horizontal, 20)
      .padding(.vertical, 16)
      Divider()

      if store.liveIsRunning, store.liveSnapshot == nil {
        VStack(spacing: 12) {
          ProgressView().controlSize(.large)
          Text("Requesting one bounded snapshot…").font(.headline)
          Text("No automatic refresh, source-link fetch, or background task will follow.")
            .font(.callout).foregroundStyle(.secondary)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
      } else if let snapshot = store.liveSnapshot {
        liveContent(snapshot)
      } else {
        ContentUnavailableView {
          Label("Live is off", systemImage: "globe.badge.chevron.backward")
        } description: {
          Text(
            "Load a single permission-gated snapshot when you need geographic context. Inquiry does not continuously track aircraft, people, traffic, or emergencies."
          )
        } actions: {
          Button("Review and load", action: store.refreshLiveEvents)
            .buttonStyle(.borderedProminent)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
      }
    }
  }

  @ViewBuilder
  private func liveContent(_ snapshot: InquiryLiveSnapshot) -> some View {
    VStack(spacing: 0) {
      HStack(spacing: 12) {
        TextField("Filter events or categories", text: $search)
          .textFieldStyle(.roundedBorder)
          .accessibilityIdentifier("inquiry.live.search")
        Text("\(filteredEvents(snapshot).count) of \(snapshot.events.count)")
          .font(.caption.monospacedDigit())
          .foregroundStyle(.secondary)
      }
      .padding(.horizontal, 20)
      .padding(.vertical, 10)
      Divider()

      GeometryReader { proxy in
        if proxy.size.width >= 880 {
          HSplitView {
            eventMap(snapshot)
              .frame(minWidth: 420)
            eventList(snapshot)
              .frame(minWidth: 360, idealWidth: 420)
          }
        } else {
          VStack(spacing: 0) {
            eventMap(snapshot)
              .frame(minHeight: 220, idealHeight: 300)
            Divider()
            eventList(snapshot)
          }
        }
      }

      Divider()
      HStack(alignment: .top, spacing: 10) {
        Image(systemName: "exclamationmark.shield")
          .foregroundStyle(.orange)
        VStack(alignment: .leading, spacing: 2) {
          Text(snapshot.statusStatement).font(.caption)
          Text(snapshot.provenance.surveillanceSafeguard)
            .font(.caption2)
            .foregroundStyle(.secondary)
        }
        Spacer()
                VStack(alignment: .trailing, spacing: 2) {
                    Text("Retrieved \(snapshot.retrievedAt.formatted(date: .abbreviated, time: .standard))")
                    Text("Approval: \(snapshot.approvalMode.replacingOccurrences(of: "_", with: " "))")
                    if let rateLimit = snapshot.providerRateLimit,
                       let remaining = rateLimit.remaining,
                       let limit = rateLimit.limit
                    {
                      Text("Provider quota: \(remaining)/\(limit) remaining")
                        .help(rateLimit.statement)
                    }
                }
                .font(.caption2.monospacedDigit())
                .foregroundStyle(.secondary)
                .help("Execution plan \(snapshot.executionPlanId)")
            }
      .padding(.horizontal, 16)
      .padding(.vertical, 10)
    }
  }

  private func eventMap(_ snapshot: InquiryLiveSnapshot) -> some View {
    Map {
      ForEach(markers(snapshot)) { marker in
        Marker(marker.title, coordinate: marker.coordinate)
          .tint(.orange)
      }
    }
    .mapStyle(.standard(elevation: .flat, emphasis: .muted))
    .accessibilityLabel("Map of provider-curated natural-event records")
    .accessibilityIdentifier("inquiry.live.map")
  }

  private func eventList(_ snapshot: InquiryLiveSnapshot) -> some View {
    List(filteredEvents(snapshot)) { event in
      VStack(alignment: .leading, spacing: 6) {
        HStack(alignment: .firstTextBaseline) {
          Text(event.title).font(.headline)
          Spacer()
          if let latest = event.geometries.map(\.sourceTimestamp).max() {
            Text(latest.formatted(date: .abbreviated, time: .shortened))
              .font(.caption2.monospacedDigit())
              .foregroundStyle(.secondary)
          }
        }
        if let magnitude = event.geometries.last?.magnitude {
          Text("Provider magnitude: \(magnitude.value.formatted()) \(magnitude.unit)")
            .font(.caption2.monospacedDigit())
            .foregroundStyle(.secondary)
        }
        Text(event.categories.map(\.title).joined(separator: " · "))
          .font(.caption)
          .foregroundStyle(.secondary)
        if let description = event.description, !description.isEmpty {
          Text(description).font(.callout).lineLimit(3)
        }
        HStack(spacing: 10) {
          Link("NASA EONET record", destination: event.eonetUrl)
          if let source = event.sources.first {
            Link("Primary source link", destination: source.url)
          }
          Text("Not independently verified")
            .foregroundStyle(.secondary)
        }
        .font(.caption)
      }
      .padding(.vertical, 6)
    }
    .listStyle(.inset)
    .accessibilityIdentifier("inquiry.live.events")
  }

  private func filteredEvents(_ snapshot: InquiryLiveSnapshot) -> [InquiryLiveEvent] {
    let needle = search.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
    guard !needle.isEmpty else { return snapshot.events }
    return snapshot.events.filter { event in
      event.title.lowercased().contains(needle)
        || event.categories.contains { $0.title.lowercased().contains(needle) }
        || event.description?.lowercased().contains(needle) == true
    }
  }

  private func markers(_ snapshot: InquiryLiveSnapshot) -> [InquiryLiveMarker] {
    filteredEvents(snapshot).compactMap { event in
      guard let position = event.geometries.reversed().compactMap(\.representativePosition).first
      else {
        return nil
      }
      return InquiryLiveMarker(
        id: event.id,
        title: event.title,
        coordinate: CLLocationCoordinate2D(
          latitude: position.latitude, longitude: position.longitude)
      )
    }
  }
}

private struct InquiryLiveMarker: Identifiable {
  let id: String
  let title: String
  let coordinate: CLLocationCoordinate2D
}

extension InquiryLiveGeometry {
  fileprivate var representativePosition: InquiryLivePosition? {
    if let position = shape.position { return position }
    guard let positions = shape.rings?.first, !positions.isEmpty else { return nil }
    let count = Double(positions.count)
    return InquiryLivePosition(
      longitude: positions.reduce(0) { $0 + $1.longitude } / count,
      latitude: positions.reduce(0) { $0 + $1.latitude } / count,
      altitude: nil
    )
  }
}

private struct ReportView: View {
  let report: InquiryReport
  let loadCitedPortraits: Bool
  let isRenderingReport: Bool
  let openInteractiveReport: () -> Void
  let exportWord: () -> Void
  let exportExcel: () -> Void

  var body: some View {
    ScrollView {
      LazyVStack(alignment: .leading, spacing: 18) {
        ViewThatFits(in: .horizontal) {
          HStack(alignment: .top, spacing: 24) {
            reportHeading
            Spacer(minLength: 20)
            evidenceAssessment
          }
          VStack(alignment: .leading, spacing: 12) {
            reportHeading
            evidenceAssessment
          }
        }
        if !report.metrics.isEmpty {
          Text("Metrics").font(.title2.bold())
          LazyVGrid(columns: [GridItem(.adaptive(minimum: 220))], spacing: 10) {
            ForEach(report.metrics) { metric in
              VStack(alignment: .leading, spacing: 8) {
                Text(metric.label).font(.caption).foregroundStyle(.secondary)
                Text("\(metric.displayValue) \(metric.unit)").font(.title3.bold()).textSelection(
                  .enabled)
                HStack {
                  Text(metric.period ?? "Period not supplied").font(.caption2).foregroundStyle(
                    .tertiary)
                  if let source = report.sources.first(where: { metric.sourceIds.contains($0.id) })
                  {
                    Link("Source", destination: source.url).font(.caption2)
                  }
                }
              }
              .frame(maxWidth: .infinity, alignment: .leading).padding(14)
              .background(.regularMaterial, in: RoundedRectangle(cornerRadius: 12))
            }
          }
        }
        if let tables = report.tables, !tables.isEmpty {
          ReportTablesView(tables: tables)
        }
        if !report.findings.isEmpty {
          Text("Findings").font(.title2.bold())
        }
        ForEach(report.findings) { finding in
          VStack(alignment: .leading, spacing: 9) {
            HStack {
              Text(finding.facet.uppercased()).font(.caption2.monospaced()).foregroundStyle(
                .secondary)
              if finding.contentTrust == "external_untrusted" {
                Text("UNTRUSTED EXCERPT").font(.caption2.monospaced()).foregroundStyle(.orange)
              }
              Spacer()
              Text("support: \(finding.confidence)").font(.caption)
            }
            Text(finding.title).font(.headline)
            if let portrait = report.sources.first(where: {
              finding.sourceIds.contains($0.id)
                && ["identity_portrait", "official_portrait"].contains($0.provenance.mediaRole)
                && safePreviewURL($0.provenance.previewUrl) != nil
            }) {
              PortraitAndBody(
                source: portrait,
                findingBody: finding.body,
                remoteMediaMode: portraitRemoteMediaMode(
                  preferenceEnabled: loadCitedPortraits,
                  reportUsedNetwork: report.run.networkUsed
                )
              )
            } else if let eventMedia = report.sources.first(where: {
              finding.sourceIds.contains($0.id)
                && $0.provenance.mediaRole == "rights_checked_event_media"
                && safePreviewURL($0.provenance.previewUrl) != nil
            }) {
              EventMediaAndBody(source: eventMedia, findingBody: finding.body)
            } else if let preview = report.sources
              .filter({ finding.sourceIds.contains($0.id) })
              .compactMap({ safePreviewURL($0.provenance.previewUrl) })
              .first
            {
              Link(destination: preview) {
                VStack(alignment: .leading, spacing: 5) {
                  Label("Open remote image preview", systemImage: "photo.badge.arrow.down")
                    .fontWeight(.semibold)
                  Text(
                    "Clicking contacts \(preview.host ?? "the external source"). Inquiry does not load remote media automatically."
                  )
                  .font(.caption)
                  .foregroundStyle(.secondary)
                }
                .frame(maxWidth: .infinity, alignment: .leading)
                .padding(12)
              }
              .buttonStyle(.plain)
              .background(.quaternary, in: RoundedRectangle(cornerRadius: 12))
              Text(finding.body).foregroundStyle(.secondary).textSelection(.enabled)
            } else {
              Text(finding.body).foregroundStyle(.secondary).textSelection(.enabled)
            }
            HStack {
              ForEach(visibleFindingTags(finding.tags), id: \.self) {
                Text($0).font(.caption2.monospaced()).padding(.horizontal, 7).padding(.vertical, 3)
                  .background(.quaternary, in: Capsule())
              }
            }
            ForEach(report.sources.filter { finding.sourceIds.contains($0.id) }) { source in
              HStack(spacing: 10) {
                Link(destination: source.url) {
                  Label(source.publisher, systemImage: "arrow.up.right.square")
                }
                if let contentURL = source.provenance.contentUrl {
                  Link("Open file", destination: contentURL)
                }
                if let previewURL = source.provenance.previewUrl {
                  Link("Preview", destination: previewURL)
                }
              }
              .font(.caption)
            }
          }
          .frame(maxWidth: .infinity, alignment: .leading)
          .padding(.vertical, 12)
          Divider()
        }
        if report.findings.isEmpty,
          report.metrics.isEmpty,
          report.tables?.isEmpty != false
        {
          ContentUnavailableView(
            "No accepted findings",
            systemImage: "questionmark.diamond",
            description: Text(
              "Inquiry returned an explicit abstention or no source met the acceptance checks. Review the summary and limits, then add a jurisdiction, date, or more specific identifier."
            )
          )
        }
        if !report.warnings.isEmpty {
          GroupBox("Warnings and limits") {
            DisclosureGroup(
              "Review \(report.warnings.count) notice\(report.warnings.count == 1 ? "" : "s")"
            ) {
              VStack(alignment: .leading, spacing: 7) {
                ForEach(report.warnings, id: \.self) {
                  Text("• \($0)").textSelection(.enabled)
                }
              }
              .frame(maxWidth: .infinity, alignment: .leading)
              .padding(.top, 6)
            }
          }
        }
        if !report.run.connectorErrors.isEmpty {
          GroupBox {
            VStack(alignment: .leading, spacing: 6) {
              Label(
                "\(report.run.connectorErrors.count) connector failure\(report.run.connectorErrors.count == 1 ? "" : "s")",
                systemImage: "exclamationmark.arrow.triangle.2.circlepath"
              )
              .fontWeight(.semibold)
              ForEach(report.run.connectorErrors, id: \.self) { error in
                Text(error)
                  .font(.caption.monospaced())
                  .textSelection(.enabled)
              }
            }
            .foregroundStyle(.orange)
            .frame(maxWidth: .infinity, alignment: .leading)
          }
        }
        GroupBox("Connector record") {
          DisclosureGroup("Show provenance ledger") {
            VStack(alignment: .leading, spacing: 7) {
              Text(
                "Engine \(report.run.engineVersion) · network \(report.run.networkUsed ? "used" : "offline")"
              )
              Text(
                "Attempted: \(report.run.connectorsAttempted.isEmpty ? "none" : report.run.connectorsAttempted.joined(separator: ", "))"
              )
              Text(
                "Succeeded: \(report.run.connectorsSucceeded.isEmpty ? "none" : report.run.connectorsSucceeded.joined(separator: ", "))"
              )
            }
            .font(.caption.monospaced())
            .textSelection(.enabled)
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(.top, 6)
          }
        }
        Text("Sources").font(.title2.bold())
        ForEach(report.sources) { source in
          VStack(alignment: .leading, spacing: 4) {
            HStack {
              Link(source.title, destination: source.url).lineLimit(2)
              Spacer()
              Image(systemName: "arrow.up.right.square").foregroundStyle(.secondary)
            }
            Text("\(source.publisher) · \(source.quality)").font(.caption).foregroundStyle(
              .secondary)
            Text(
              "\(sourceProvenanceTime(source))\(source.provenance.observationPeriod.map { " · observation \($0)" } ?? "")\(source.publishedAt.map { " · published \($0)" } ?? "")"
            )
            .font(.caption2).foregroundStyle(.tertiary)
            if let license = source.license {
              Text(license).font(.caption2).foregroundStyle(.tertiary).lineLimit(2)
            }
            if let fileFormat = source.provenance.fileFormat {
              Text(
                "File \(fileFormat)\(source.provenance.fileSizeBytes.map { " · \($0) bytes" } ?? "")"
              )
              .font(.caption2).foregroundStyle(.tertiary)
            }
            HStack {
              if let contentURL = source.provenance.contentUrl {
                Link("Open file", destination: contentURL)
              }
              if let previewURL = source.provenance.previewUrl {
                Link("Preview", destination: previewURL)
              }
            }
            .font(.caption2)
          }
          .padding(.vertical, 5)
        }
        HStack {
          Button(action: openInteractiveReport) {
            if isRenderingReport {
              Label("Preparing interactive dossier…", systemImage: "doc.richtext")
            } else {
              Label("Open interactive dossier", systemImage: "doc.richtext")
            }
          }
          .buttonStyle(.borderedProminent)
          .controlSize(.large)
          .disabled(isRenderingReport)
          .accessibilityIdentifier("inquiry.report.openInteractive")
          Menu("Export", systemImage: "square.and.arrow.up") {
            Button("Word-compatible RTF…", action: exportWord)
            Button("Excel-compatible CSV…", action: exportExcel)
          }
          .controlSize(.large)
        }
        .padding(.top, 8)
      }
      .padding(28)
    }
  }

  private var reportHeading: some View {
    VStack(alignment: .leading, spacing: 6) {
      Text(report.query)
        .font(.largeTitle.bold())
        .textSelection(.enabled)
        .accessibilityAddTraits(.isHeader)
      Text(report.summary)
        .font(.body)
        .foregroundStyle(.secondary)
        .textSelection(.enabled)
    }
  }

  private var evidenceAssessment: some View {
    VStack(alignment: .leading, spacing: 8) {
      Label(
        report.evidence?.label ?? "\(report.confidence.capitalized) source coverage",
        systemImage: evidenceStatusIcon
      )
      .font(.subheadline.weight(.semibold))
      Text(
        report.evidence?.explanation ?? "This describes source coverage, not answer correctness."
      )
      .font(.caption)
      .foregroundStyle(.secondary)
      .fixedSize(horizontal: false, vertical: true)
      if let evidence = report.evidence {
        ViewThatFits(in: .horizontal) {
          HStack(spacing: 14) { evidenceDimensions(evidence) }
          VStack(alignment: .leading, spacing: 4) { evidenceDimensions(evidence) }
        }
        .font(.caption2.monospaced())
        .foregroundStyle(.secondary)
      }
    }
    .frame(maxWidth: 460, alignment: .leading)
    .accessibilityIdentifier("inquiry.report.evidenceAssessment")
  }

  @ViewBuilder
  private func evidenceDimensions(_ evidence: InquiryEvidenceAssessment) -> some View {
    Text("coverage \(evidence.sourceCoverage)")
    Text("publishers \(evidence.publisherDiversity)")
    if evidence.freshness != "not_applicable" { Text("freshness \(evidence.freshness)") }
    if evidence.identityBinding != "not_applicable" { Text("identity \(evidence.identityBinding)") }
    if evidence.mediaRights != "not_applicable" { Text("media rights \(evidence.mediaRights)") }
  }

  private var evidenceStatusIcon: String {
    switch report.evidence?.status {
    case "verified_identity": "checkmark.seal.fill"
    case "evidence_available": "doc.text.magnifyingglass"
    case "partial": "circle.lefthalf.filled"
    case "abstained": "hand.raised.fill"
    default: "checkmark.seal"
    }
  }
}

func visibleFindingTags(_ tags: [String]) -> [String] {
  tags.compactMap { tag in
    guard !tag.hasPrefix("event-match:") else { return nil }
    return tag.replacingOccurrences(of: "_", with: " ")
      .replacingOccurrences(of: "-", with: " ")
  }
}

private struct EventMediaAndBody: View {
  let source: InquirySource
  let findingBody: String

  var body: some View {
    ViewThatFits(in: .horizontal) {
      HStack(alignment: .top, spacing: 18) {
        EventMediaCard(source: source)
          .frame(minWidth: 300, idealWidth: 420, maxWidth: 520, alignment: .leading)
        Text(findingBody)
          .foregroundStyle(.secondary)
          .textSelection(.enabled)
          .fixedSize(horizontal: false, vertical: true)
          .frame(minWidth: 300, maxWidth: .infinity, alignment: .leading)
      }
      VStack(alignment: .leading, spacing: 12) {
        EventMediaCard(source: source)
        Text(findingBody).foregroundStyle(.secondary).textSelection(.enabled)
      }
    }
  }
}

private struct EventMediaCard: View {
  let source: InquirySource
  @StateObject private var loader = RemotePortraitLoader()
  @State private var approved = false

  var body: some View {
    VStack(alignment: .leading, spacing: 9) {
      Group {
        if !approved {
          ContentUnavailableView {
            Label("Cited event media is off", systemImage: "photo.on.rectangle.angled")
          } description: {
            Text(
              "This Commons file has accepted machine-readable reuse terms, but Inquiry will not contact the image host until you approve this preview. Subject and recency remain discovery claims."
            )
          } actions: {
            Button("Load in Inquiry") {
              approved = true
              loader.load(source: source)
            }
            .buttonStyle(.borderedProminent)
          }
        } else {
          switch loader.state {
          case .idle, .loading:
            ZStack {
              Rectangle().fill(.quaternary)
              ProgressView("Loading approved preview…")
            }
          case .loaded(let image):
            Image(nsImage: image)
              .resizable()
              .scaledToFit()
              .accessibilityLabel(source.provenance.altText ?? source.title)
          case .failed(let message):
            ContentUnavailableView {
              Label("Media unavailable", systemImage: "photo.badge.exclamationmark")
            } description: {
              Text(message)
            } actions: {
              Button("Retry") { loader.retry(source: source) }
              Button("Turn off") {
                loader.cancel()
                approved = false
              }
            }
          }
        }
      }
      .frame(maxWidth: .infinity, minHeight: 240, maxHeight: 460)
      .clipShape(RoundedRectangle(cornerRadius: 12))
      .accessibilityIdentifier(approved ? "inquiry.media.approved" : "inquiry.media.off")

      VStack(alignment: .leading, spacing: 3) {
        Text(source.provenance.altText ?? source.title)
          .font(.caption)
          .fontWeight(.semibold)
        Text(eventMediaAttribution)
          .font(.caption2)
          .foregroundStyle(.secondary)
          .textSelection(.enabled)
        HStack(spacing: 10) {
          Link("Source and rights", destination: source.url)
          if let preview = safePreviewURL(source.provenance.previewUrl) {
            Link("Open preview", destination: preview)
          }
        }
        .font(.caption2)
      }
    }
    .padding(12)
    .background(.quaternary.opacity(0.45), in: RoundedRectangle(cornerRadius: 14))
    .onDisappear { loader.cancel() }
  }

  private var eventMediaAttribution: String {
    let creator = source.provenance.creator ?? "Creator not supplied"
    let license = source.license ?? "Check source terms"
    return "\(creator) · \(license)"
  }
}

enum PortraitRemoteMediaMode: Hashable {
  case automatic
  case disabledInSettings
  case offlineReport
}

func portraitRemoteMediaMode(
  preferenceEnabled: Bool,
  reportUsedNetwork: Bool
) -> PortraitRemoteMediaMode {
  if !reportUsedNetwork {
    .offlineReport
  } else if preferenceEnabled {
    .automatic
  } else {
    .disabledInSettings
  }
}

private struct PortraitAndBody: View {
  let source: InquirySource
  let findingBody: String
  let remoteMediaMode: PortraitRemoteMediaMode

  var body: some View {
    ViewThatFits(in: .horizontal) {
      HStack(alignment: .top, spacing: 18) {
        PortraitCard(source: source, remoteMediaMode: remoteMediaMode)
          .frame(width: 280, alignment: .leading)
        Text(findingBody)
          .foregroundStyle(.secondary)
          .textSelection(.enabled)
          .fixedSize(horizontal: false, vertical: true)
          .frame(minWidth: 340, maxWidth: .infinity, alignment: .leading)
      }
      .frame(maxWidth: .infinity, alignment: .leading)
      VStack(alignment: .leading, spacing: 12) {
        PortraitCard(source: source, remoteMediaMode: remoteMediaMode)
        Text(findingBody).foregroundStyle(.secondary).textSelection(.enabled)
      }
    }
  }
}

private struct PortraitCard: View {
  let source: InquirySource
  let remoteMediaMode: PortraitRemoteMediaMode
  @StateObject private var loader = RemotePortraitLoader()

  var body: some View {
    VStack(alignment: .leading, spacing: 9) {
      Group {
        if remoteMediaMode == .automatic {
          switch loader.state {
          case .idle, .loading:
            ZStack {
              RoundedRectangle(cornerRadius: 12).fill(.quaternary)
              ProgressView("Loading cited portrait…")
            }
          case .loaded(let image):
            Image(nsImage: image)
              .resizable()
              .scaledToFit()
              .accessibilityLabel(source.provenance.altText ?? source.title)
          case .failed(let message):
            ContentUnavailableView {
              Label("Portrait unavailable", systemImage: "photo.badge.exclamationmark")
            } description: {
              Text(message)
            } actions: {
              Button("Retry") {
                loader.retry(source: source)
              }
            }
          }
        } else if remoteMediaMode == .offlineReport {
          ContentUnavailableView(
            "Offline result",
            systemImage: "network.slash",
            description: Text(
              "This report did not contact remote media. The cited source and preview links remain available below."
            )
          )
        } else {
          ContentUnavailableView {
            Label("Portrait previews are disabled", systemImage: "photo")
          } description: {
            Text(
              "Enable automatic cited portrait previews in Settings to load this rights-checked Wikimedia image."
            )
          } actions: {
            SettingsLink {
              Text("Open Settings")
            }
          }
        }
      }
      .frame(maxWidth: 260, minHeight: 220, maxHeight: 340)
      .aspectRatio(citedAspectRatio, contentMode: .fit)
      .clipShape(RoundedRectangle(cornerRadius: 12))
      .accessibilityIdentifier(portraitStateIdentifier)

      VStack(alignment: .leading, spacing: 3) {
        Text(source.provenance.altText ?? source.title)
          .font(.caption)
          .fontWeight(.semibold)
        Text(attribution)
          .font(.caption2)
          .foregroundStyle(.secondary)
          .textSelection(.enabled)
        HStack(spacing: 10) {
          Link("Source and rights", destination: source.url)
          if let preview = safePreviewURL(source.provenance.previewUrl) {
            Link("Open preview", destination: preview)
          }
        }
        .font(.caption2)
      }
    }
    .padding(12)
    .background(.quaternary.opacity(0.45), in: RoundedRectangle(cornerRadius: 14))
    .task(id: loadTaskID) {
      loader.cancel()
      if remoteMediaMode == .automatic {
        loader.load(source: source)
      }
    }
    .onDisappear { loader.cancel() }
  }

  private var loadTaskID: String {
    "\(source.id)|\(source.provenance.previewUrl?.absoluteString ?? "none")|\(remoteMediaMode)"
  }

  private var portraitStateIdentifier: String {
    switch remoteMediaMode {
    case .disabledInSettings:
      "inquiry.portrait.disabled"
    case .offlineReport:
      "inquiry.portrait.offline"
    case .automatic:
      switch loader.state {
      case .idle, .loading: "inquiry.portrait.loading"
      case .loaded: "inquiry.portrait.loaded"
      case .failed: "inquiry.portrait.failed"
      }
    }
  }

  private var citedAspectRatio: CGFloat {
    guard let width = source.provenance.widthPixels,
      let height = source.provenance.heightPixels,
      width > 0, height > 0
    else { return 0.78 }
    return CGFloat(width) / CGFloat(height)
  }

  private var attribution: String {
    let creator = source.provenance.creator ?? "Creator not supplied"
    let credit = source.provenance.credit.map { " · Credit: \($0)" } ?? ""
    let license = source.license.map { " · \($0)" } ?? ""
    let dimensions: String
    if let width = source.provenance.widthPixels, let height = source.provenance.heightPixels {
      dimensions = " · \(width)×\(height)"
    } else {
      dimensions = ""
    }
    return "Creator: \(creator)\(credit)\(license)\(dimensions)"
  }
}

private struct SensitiveQueryReview: View {
  let assessment: InquiryPrivacyAssessment
  let originalQuery: String
  @Binding var originalSendAcknowledged: Bool
  let useOffline: () -> Void
  let sendRedacted: () -> Void
  let sendOriginal: () -> Void
  let cancel: () -> Void

  private let destinations =
    "Wikipedia/Wikimedia, Wikidata, MedlinePlus, OpenAlex, Open Library, World Bank, NASA Science, Wikimedia Commons, and a configured SearXNG service when applicable."

  var body: some View {
    VStack(alignment: .leading, spacing: 18) {
      Label("Sensitive query review", systemImage: "network.badge.shield.half.filled")
        .font(.title2.bold())
      Text(
        "Inquiry stopped before networking. Detected categories: \(assessment.indicators.joined(separator: ", "))."
      )
      GroupBox("Original query — not sent") {
        Text(originalQuery)
          .textSelection(.enabled)
          .frame(maxWidth: .infinity, alignment: .leading)
          .padding(.top, 4)
      }
      GroupBox("Redacted query candidate") {
        Text(assessment.redactedQuery)
          .textSelection(.enabled)
          .frame(maxWidth: .infinity, alignment: .leading)
          .padding(.top, 4)
      }
      GroupBox("Potential public destinations") {
        Text(destinations)
          .textSelection(.enabled)
          .frame(maxWidth: .infinity, alignment: .leading)
          .padding(.top, 4)
      }
      Text(assessment.guidance).foregroundStyle(.secondary)
      if assessment.level == "sensitive" {
        Toggle(
          "I understand that sending the original will disclose its exact text to applicable public services.",
          isOn: $originalSendAcknowledged)
      }
      HStack {
        Button("Keep offline", action: useOffline)
        if assessment.redactedQuerySafeToSend {
          Button("Send redacted", action: sendRedacted)
            .buttonStyle(.borderedProminent)
        }
        Spacer()
        Button("Cancel", role: .cancel, action: cancel)
        if assessment.level == "sensitive" {
          Button("Send exact original", role: .destructive, action: sendOriginal)
            .disabled(!originalSendAcknowledged)
        }
      }
    }
    .padding(24)
    .frame(minWidth: 620, idealWidth: 720)
  }
}

private func safePreviewURL(_ value: URL?) -> URL? {
  guard let value, value.scheme == "https" else { return nil }
  guard ["upload.wikimedia.org", "commons.wikimedia.org"].contains(value.host) else {
    return nil
  }
  return value
}

func sourceProvenanceTime(_ source: InquirySource) -> String {
  if source.quality == "discovery_only" && source.provenance.requestUrl == nil {
    if let value = source.provenance.sourceUpdatedAt,
      value.hasPrefix("curated registry reviewed ")
    {
      return
        "Registry reviewed \(value.dropFirst("curated registry reviewed ".count)); not retrieved in this run"
    }
    return "Not retrieved in this run"
  }
  return "Retrieved \(source.retrievedAt.formatted(date: .abbreviated, time: .shortened))"
}

func shouldResetOriginalSendAcknowledgement() -> Bool {
  true
}
