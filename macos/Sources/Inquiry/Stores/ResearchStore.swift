import AppKit
import Foundation
import UniformTypeIdentifiers

@MainActor
final class ResearchStore: ObservableObject {
  @Published var query = ""
  @Published var report: InquiryReport?
  @Published var isRunning = false
  @Published private(set) var isRenderingReport = false
  @Published private(set) var interactiveReportURL: URL?
  @Published var errorMessage: String?
  @Published var offline = false
  @Published var privacyAssessment: InquiryPrivacyAssessment?
  @Published var pendingExecutionPlan: InquiryExecutionPlan?
  @Published var pendingLiveExecutionPlan: InquiryExecutionPlan?
  @Published var focusRequestID = UUID()
  @Published private(set) var recentQueries: [String] = []
  @Published var studyDirectory: URL?
  @Published var studyCourse = ""
  @Published var studyInstructor = ""
  @Published var includeSpeakerNotes = false
  @Published var studyMaterialAuthorized = false
  @Published var studyQuery = ""
  @Published var studyIsRunning = false
  @Published var studyIndexSummary: StudyIndexSummary?
  @Published var studySearch: LocalStudySearch?
  @Published var studyExportMessage: String?
  @Published private(set) var liveSnapshot: InquiryLiveSnapshot?
  @Published private(set) var liveIsRunning = false

  private var process: (any InquiryProcessing)?
  private var reportData: Data?
  private var researchTask: Task<Void, Never>?
  private var renderTask: Task<Void, Never>?
  private var studyTask: Task<Void, Never>?
  private var liveTask: Task<Void, Never>?
  private var activeRunID: UUID?
  private var pendingExecution: PendingExecution?
  private let permissionMode: @Sendable () -> InquiryConnectorPermissionMode

  init(
    process: (any InquiryProcessing)? = try? InquiryProcess(),
    permissionMode: @escaping @Sendable () -> InquiryConnectorPermissionMode = {
      InquiryConnectorPermissionMode(
        rawValue: UserDefaults.standard.string(
          forKey: InquiryPermissionPreferences.connectorMode
        ) ?? ""
      ) ?? .askEveryTime
    }
  ) {
    self.process = process
    self.permissionMode = permissionMode
    Task { await process?.cleanupRenderedReports() }
  }

  func research() {
    beginResearch(
      preflight: true,
      redactSensitive: false,
      confirmSensitiveWeb: false,
      suppressHistory: false
    )
  }

  func useOfflineForSensitiveQuery() {
    offline = true
    privacyAssessment = nil
    beginResearch(
      preflight: false,
      redactSensitive: false,
      confirmSensitiveWeb: false,
      suppressHistory: true
    )
  }

  func sendRedactedSensitiveQuery() {
    guard let assessment = privacyAssessment, assessment.redactedQuerySafeToSend else { return }
    query = assessment.redactedQuery
    privacyAssessment = nil
    beginResearch(
      preflight: false,
      redactSensitive: false,
      confirmSensitiveWeb: false,
      suppressHistory: true
    )
  }

  func confirmSensitiveWebQuery() {
    privacyAssessment = nil
    beginResearch(
      preflight: false,
      redactSensitive: false,
      confirmSensitiveWeb: true,
      suppressHistory: true
    )
  }

  private func beginResearch(
    preflight: Bool,
    redactSensitive: Bool,
    confirmSensitiveWeb: Bool,
    suppressHistory: Bool
  ) {
    let trimmed = query.trimmingCharacters(in: .whitespacesAndNewlines)
    guard trimmed.count >= 3, !isRunning else { return }
    guard let process else {
      errorMessage = InquiryProcessError.binaryMissing.localizedDescription
      return
    }
    isRunning = true
    errorMessage = nil
    let runID = UUID()
    activeRunID = runID
    researchTask = Task {
      do {
        var privacyFlagged = suppressHistory
        if preflight {
          let assessment = try await process.privacyCheck(query: trimmed)
          guard activeRunID == runID else { return }
          if assessment.requiresNetworkConfirmation {
            privacyFlagged = true
            if !offline {
              privacyAssessment = assessment
              isRunning = false
              researchTask = nil
              return
            }
          }
        }
        let selectedPermissionMode = permissionMode()
        let effectiveOffline = offline || selectedPermissionMode == .offlineOnly
        let executionPlan = try await process.plan(query: trimmed)
        guard activeRunID == runID else { return }
        if executionPlan.intent.clarification != nil {
          pendingExecutionPlan = executionPlan
          pendingExecution = PendingExecution(
            query: trimmed,
            offline: true,
            redactSensitive: redactSensitive,
            confirmSensitiveWeb: confirmSensitiveWeb,
            suppressHistory: suppressHistory,
            privacyFlagged: privacyFlagged
          )
          errorMessage = nil
          isRunning = false
          researchTask = nil
          activeRunID = nil
          return
        }
        if !effectiveOffline,
          executionPlan.permissionRequired,
          !(selectedPermissionMode == .automaticPublicWeb && executionPlan.automaticEligible)
        {
          pendingExecutionPlan = executionPlan
          pendingExecution = PendingExecution(
            query: trimmed,
            offline: false,
            redactSensitive: redactSensitive,
            confirmSensitiveWeb: confirmSensitiveWeb,
            suppressHistory: suppressHistory,
            privacyFlagged: privacyFlagged
          )
          isRunning = false
          researchTask = nil
          activeRunID = nil
          return
        }
        let result = try await process.research(
          query: trimmed,
          offline: effectiveOffline,
          redactSensitive: redactSensitive,
          confirmSensitiveWeb: confirmSensitiveWeb,
          approvedPlanID: nil,
          automaticPublicWeb: !effectiveOffline
            && selectedPermissionMode == .automaticPublicWeb
            && executionPlan.automaticEligible
        )
        guard activeRunID == runID else { return }
        report = result.report
        reportData = result.data
        if shouldStoreRecentQuery(
          privacyFlagged: privacyFlagged,
          redactSensitive: redactSensitive,
          confirmSensitiveWeb: confirmSensitiveWeb
        ) {
          recentQueries.removeAll { $0 == result.report.query }
          recentQueries.insert(result.report.query, at: 0)
          recentQueries = Array(recentQueries.prefix(12))
        }
      } catch is CancellationError {
        if activeRunID == runID {
          errorMessage = nil
        }
      } catch {
        if activeRunID == runID {
          errorMessage = error.localizedDescription
        }
      }
      if activeRunID == runID {
        isRunning = false
        researchTask = nil
        activeRunID = nil
      }
    }
  }

  func approvePendingExecutionPlan() {
    guard let plan = pendingExecutionPlan,
      plan.permissionRequired,
      let pendingExecution,
      let process,
      !isRunning
    else { return }
    self.pendingExecutionPlan = nil
    self.pendingExecution = nil
    errorMessage = nil
    isRunning = true
    let runID = UUID()
    activeRunID = runID
    researchTask = Task {
      do {
        let result = try await process.research(
          query: pendingExecution.query,
          offline: pendingExecution.offline,
          redactSensitive: pendingExecution.redactSensitive,
          confirmSensitiveWeb: pendingExecution.confirmSensitiveWeb,
          approvedPlanID: plan.planId,
          automaticPublicWeb: false
        )
        guard activeRunID == runID else { return }
        report = result.report
        reportData = result.data
        if shouldStoreRecentQuery(
          privacyFlagged: pendingExecution.privacyFlagged,
          redactSensitive: pendingExecution.redactSensitive,
          confirmSensitiveWeb: pendingExecution.confirmSensitiveWeb
        ) {
          recentQueries.removeAll { $0 == result.report.query }
          recentQueries.insert(result.report.query, at: 0)
          recentQueries = Array(recentQueries.prefix(12))
        }
      } catch is CancellationError {
        if activeRunID == runID { errorMessage = nil }
      } catch {
        if activeRunID == runID { errorMessage = error.localizedDescription }
      }
      if activeRunID == runID {
        isRunning = false
        researchTask = nil
        activeRunID = nil
      }
    }
  }

  func runPendingExecutionOffline() {
    guard var pendingExecution else { return }
    pendingExecution.offline = true
    query = pendingExecution.query
    self.pendingExecution = nil
    pendingExecutionPlan = nil
    offline = true
    beginResearch(
      preflight: false,
      redactSensitive: pendingExecution.redactSensitive,
      confirmSensitiveWeb: false,
      suppressHistory: pendingExecution.suppressHistory
    )
  }

  func dismissPendingExecutionPlan() {
    pendingExecutionPlan = nil
    pendingExecution = nil
  }

  func refreshLiveEvents() {
    guard !liveIsRunning, let process else { return }
    let selectedPermissionMode = permissionMode()
    guard selectedPermissionMode != .offlineOnly else {
      errorMessage =
        "The Live workspace is unavailable in Always offline mode. Inquiry did not contact NASA or load map tiles."
      return
    }
    liveIsRunning = true
    errorMessage = nil
    liveTask = Task {
      do {
        let plan = try await process.plan(query: liveWorkspaceQuery)
        if plan.permissionRequired,
          !(selectedPermissionMode == .automaticPublicWeb && plan.automaticEligible)
        {
          pendingLiveExecutionPlan = plan
          liveIsRunning = false
          liveTask = nil
          return
        }
        liveSnapshot = try await process.liveEvents(
          approvedPlanID: nil,
          automaticPublicWeb: selectedPermissionMode == .automaticPublicWeb
            && plan.automaticEligible
        )
      } catch is CancellationError {
        errorMessage = nil
      } catch {
        errorMessage = error.localizedDescription
      }
      liveIsRunning = false
      liveTask = nil
    }
  }

  func approvePendingLiveExecutionPlan() {
    guard let plan = pendingLiveExecutionPlan,
      plan.permissionRequired,
      !liveIsRunning,
      let process
    else { return }
    pendingLiveExecutionPlan = nil
    liveIsRunning = true
    errorMessage = nil
    liveTask = Task {
      do {
        liveSnapshot = try await process.liveEvents(
          approvedPlanID: plan.planId,
          automaticPublicWeb: false
        )
      } catch is CancellationError {
        errorMessage = nil
      } catch {
        errorMessage = error.localizedDescription
      }
      liveIsRunning = false
      liveTask = nil
    }
  }

  func dismissPendingLiveExecutionPlan() {
    pendingLiveExecutionPlan = nil
  }

  func cancelLiveEvents() {
    liveTask?.cancel()
  }

  func cancelResearch() {
    researchTask?.cancel()
  }

  func startNewInquiry() {
    let oldInteractiveReport = interactiveReportURL
    cancelResearch()
    activeRunID = nil
    researchTask = nil
    renderTask?.cancel()
    renderTask = nil
    query = ""
    report = nil
    reportData = nil
    errorMessage = nil
    privacyAssessment = nil
    pendingExecutionPlan = nil
    pendingLiveExecutionPlan = nil
    pendingExecution = nil
    isRunning = false
    isRenderingReport = false
    interactiveReportURL = nil
    liveTask?.cancel()
    liveTask = nil
    liveIsRunning = false
    focusRequestID = UUID()
    if let oldInteractiveReport {
      Task { await process?.discardRenderedReport(at: oldInteractiveReport) }
    }
  }

  func selectRecent(_ value: String) {
    query = value
    focusRequestID = UUID()
  }

  func openInteractiveReport() {
    guard let report,
      let reportData,
      !isRunning,
      !isRenderingReport,
      let process
    else { return }
    isRenderingReport = true
    renderTask = Task {
      do {
        let url = try await process.render(reportData: reportData, reportID: report.id)
        try Task.checkCancellation()
        interactiveReportURL = url
      } catch is CancellationError {
        errorMessage = nil
      } catch {
        errorMessage = error.localizedDescription
      }
      isRenderingReport = false
      renderTask = nil
    }
  }

  func closeInteractiveReport() {
    guard let url = interactiveReportURL else { return }
    interactiveReportURL = nil
    Task { await process?.discardRenderedReport(at: url) }
  }

  func exportReportForWord() {
    guard let report else { return }
    let panel = NSSavePanel()
    panel.title = "Export a Word-compatible dossier"
    panel.message =
      "Inquiry writes a local RTF document. A cloud-backed destination may sync it outside this Mac."
    panel.nameFieldStringValue = safeExportName(report.query, extension: "rtf")
    panel.allowedContentTypes = [.rtf]
    guard panel.runModal() == .OK, let destination = panel.url else { return }
    do {
      try reportRTFData(report).write(to: destination, options: .atomic)
    } catch {
      errorMessage = "The Word-compatible export failed: \(error.localizedDescription)"
    }
  }

  func exportReportForExcel() {
    guard let report else { return }
    let panel = NSSavePanel()
    panel.title = "Export an Excel-compatible evidence table"
    panel.message =
      "Inquiry writes local UTF-8 CSV with formula-triggering text neutralized. A cloud-backed destination may sync it outside this Mac."
    panel.nameFieldStringValue = safeExportName(report.query, extension: "csv")
    panel.allowedContentTypes = [.commaSeparatedText]
    guard panel.runModal() == .OK, let destination = panel.url else { return }
    do {
      try reportCSVData(report).write(to: destination, options: .atomic)
    } catch {
      errorMessage = "The Excel-compatible export failed: \(error.localizedDescription)"
    }
  }

  func chooseStudyDirectory() {
    let panel = NSOpenPanel()
    panel.title = "Choose one course folder"
    panel.message =
      "Inquiry reads supported documents in this folder locally. It does not scan other folders."
    panel.prompt = "Use Course Folder"
    panel.canChooseDirectories = true
    panel.canChooseFiles = false
    panel.allowsMultipleSelection = false
    panel.resolvesAliases = false
    guard panel.runModal() == .OK, let selected = panel.url else { return }
    let standardized = selected.standardizedFileURL
    if standardized.path == "/"
      || standardized.path
        == FileManager.default.homeDirectoryForCurrentUser.standardizedFileURL.path
    {
      errorMessage =
        "Choose a specific course folder, not the filesystem root or your entire home folder."
      return
    }
    studyDirectory = standardized
    studyMaterialAuthorized = false
    studySearch = nil
    studyExportMessage = nil
  }

  func buildStudyIndex() {
    guard !studyIsRunning,
      studyMaterialAuthorized,
      let studyDirectory,
      let process
    else { return }
    studyIsRunning = true
    studySearch = nil
    studyExportMessage = nil
    errorMessage = nil
    studyTask = Task {
      do {
        let outputDirectory = try privateStudyIndexDirectory()
        let base = safeStudyPrefix(
          studyCourse.isEmpty ? studyDirectory.lastPathComponent : studyCourse
        )
        let output =
          outputDirectory
          .appendingPathComponent("\(base)-\(UUID().uuidString.lowercased())-study-index.json")
        let oldIndex = studyIndexURL
        let summary = try await process.indexStudy(
          request: StudyIndexRequest(
            directory: studyDirectory.path,
            out: output.path,
            course: optionalStudyLabel(studyCourse),
            instructor: optionalStudyLabel(studyInstructor),
            includeSpeakerNotes: includeSpeakerNotes
          )
        )
        studyIndexSummary = summary
        if let oldIndex, oldIndex != URL(fileURLWithPath: summary.path) {
          try? FileManager.default.removeItem(at: oldIndex)
        }
      } catch is CancellationError {
        errorMessage = nil
      } catch {
        errorMessage = error.localizedDescription
      }
      studyIsRunning = false
      studyTask = nil
    }
  }

  func searchStudyIndex() {
    let trimmed = studyQuery.trimmingCharacters(in: .whitespacesAndNewlines)
    guard trimmed.count >= 2,
      !studyIsRunning,
      let indexURL = studyIndexURL,
      let process
    else { return }
    studyIsRunning = true
    studyExportMessage = nil
    errorMessage = nil
    studyTask = Task {
      do {
        studySearch = try await process.searchStudy(
          indexURL: indexURL,
          query: trimmed,
          limit: 12
        )
      } catch is CancellationError {
        errorMessage = nil
      } catch {
        errorMessage = error.localizedDescription
      }
      studyIsRunning = false
      studyTask = nil
    }
  }

  func exportStudyPack() {
    guard !studyIsRunning,
      let search = studySearch,
      !search.results.isEmpty,
      let indexURL = studyIndexURL,
      let process
    else { return }
    let panel = NSOpenPanel()
    panel.title = "Choose a private export folder"
    panel.message =
      "Inquiry creates Anki CSV, Quizlet TSV, Markdown, and JSON files. Existing files are never overwritten."
    panel.prompt = "Export Here"
    panel.canChooseDirectories = true
    panel.canChooseFiles = false
    panel.allowsMultipleSelection = false
    guard panel.runModal() == .OK, let outputDirectory = panel.url else { return }
    studyIsRunning = true
    studyExportMessage = nil
    errorMessage = nil
    let prefix = safeStudyPrefix(studyCourse.isEmpty ? "inquiry-study" : studyCourse)
    studyTask = Task {
      do {
        let files = try await process.exportStudyPack(
          indexURL: indexURL,
          query: search.query,
          limit: min(search.results.count, 30),
          outputDirectory: outputDirectory,
          prefix: prefix
        )
        studyExportMessage =
          "Created \(URL(fileURLWithPath: files.ankiCsv).lastPathComponent), \(URL(fileURLWithPath: files.quizletTsv).lastPathComponent), \(URL(fileURLWithPath: files.markdown).lastPathComponent), and \(URL(fileURLWithPath: files.json).lastPathComponent)."
        NSWorkspace.shared.activateFileViewerSelecting([outputDirectory])
      } catch is CancellationError {
        errorMessage = nil
      } catch {
        errorMessage = error.localizedDescription
      }
      studyIsRunning = false
      studyTask = nil
    }
  }

  func cancelStudy() {
    studyTask?.cancel()
  }

  func revealStudyIndex() {
    guard let studyIndexURL else { return }
    NSWorkspace.shared.activateFileViewerSelecting([studyIndexURL])
  }

  func deleteStudyIndex() {
    cancelStudy()
    guard let studyIndexURL else { return }
    do {
      if FileManager.default.fileExists(atPath: studyIndexURL.path) {
        try FileManager.default.removeItem(at: studyIndexURL)
      }
      studyIndexSummary = nil
      studySearch = nil
      studyExportMessage = nil
    } catch {
      errorMessage =
        "Inquiry could not delete the private study index: \(error.localizedDescription)"
    }
  }

  var studyIndexURL: URL? {
    studyIndexSummary.map { URL(fileURLWithPath: $0.path) }
  }

  private func privateStudyIndexDirectory() throws -> URL {
    guard
      let applicationSupport = FileManager.default.urls(
        for: .applicationSupportDirectory,
        in: .userDomainMask
      ).first
    else {
      throw InquiryProcessError.failed("Application Support is unavailable")
    }
    let directory =
      applicationSupport
      .appendingPathComponent("BarnLabs", isDirectory: true)
      .appendingPathComponent("Inquiry", isDirectory: true)
      .appendingPathComponent("Study Indexes", isDirectory: true)
    try FileManager.default.createDirectory(
      at: directory,
      withIntermediateDirectories: true,
      attributes: [.posixPermissions: 0o700]
    )
    try? FileManager.default.setAttributes(
      [.posixPermissions: 0o700],
      ofItemAtPath: directory.path
    )
    return directory
  }
}

let liveWorkspaceQuery = "Show the Live workspace map of current NASA EONET natural-event records"

func safeExportName(_ query: String, extension pathExtension: String) -> String {
  let slug = query.lowercased().unicodeScalars.map { scalar -> Character in
    CharacterSet.alphanumerics.contains(scalar) ? Character(String(scalar)) : "-"
  }
  var value = String(slug)
  while value.contains("--") { value = value.replacingOccurrences(of: "--", with: "-") }
  value = String(value.trimmingCharacters(in: CharacterSet(charactersIn: "-")).prefix(52))
  if value.isEmpty { value = "inquiry-report" }
  return "\(value).\(pathExtension)"
}

func reportCSVData(_ report: InquiryReport) -> Data {
  let csv =
    reportCSVRows(report)
    .map { $0.map(inquiryCSVCell).joined(separator: ",") }
    .joined(separator: "\r\n") + "\r\n"
  return Data(csv.utf8)
}

func reportCSVRows(_ report: InquiryReport) -> [[String]] {
  let header = [
    "record_type",
    "title",
    "content",
    "facet_or_tier",
    "support",
    "publisher",
    "url",
    "source_ids_or_time",
    "license",
    "table_id",
    "row_id",
    "column_key",
    "column_label",
    "unit",
    "notes",
  ]
  let emptyTableFields = Array(repeating: "", count: 6)
  var rows = [header]
  rows += report.findings.map { finding in
    [
      "finding",
      finding.title,
      finding.body,
      finding.facet,
      finding.confidence,
      "",
      "",
      finding.sourceIds.joined(separator: ";"),
      "",
    ] + emptyTableFields
  }
  rows += report.metrics.map { metric in
    [
      "metric",
      metric.label,
      "\(metric.displayValue) \(metric.unit)",
      metric.facet,
      "",
      "",
      "",
      metric.sourceIds.joined(separator: ";")
        + (metric.period.map { ";period=\($0)" } ?? ""),
      "",
    ] + emptyTableFields
  }
  for table in report.tables ?? [] {
    rows.append([
      "table",
      table.title,
      table.description,
      "reference_table",
      "\(table.rows.count) rows",
      "",
      "",
      table.sourceIds.joined(separator: ";"),
      "",
      table.id,
      "",
      "",
      "",
      "",
      table.notes.joined(separator: " | "),
    ])

    let columns = inquiryTableDisplayColumns(table)
    for row in table.rows {
      for column in columns {
        rows.append([
          "table_cell",
          table.title,
          inquiryTableCell(row, at: column.index),
          "",
          "",
          "",
          "",
          table.sourceIds.joined(separator: ";"),
          "",
          table.id,
          row.id,
          column.key,
          column.label,
          column.unit ?? "",
          "",
        ])
      }
    }
  }
  rows += report.sources.map { source in
    [
      "source",
      source.title,
      "",
      source.quality,
      "",
      source.publisher,
      source.url.absoluteString,
      ISO8601DateFormatter().string(from: source.retrievedAt),
      source.license ?? "Check source terms",
    ] + emptyTableFields
  }
  return rows
}

func inquiryCSVCell(_ value: String) -> String {
  let firstVisible = value.unicodeScalars.first { scalar in
    !CharacterSet.whitespacesAndNewlines.contains(scalar)
      && ![
        "\u{feff}", "\u{200b}", "\u{200c}", "\u{200d}", "\u{200e}", "\u{200f}", "\u{202a}",
        "\u{202b}", "\u{202c}", "\u{202d}", "\u{202e}", "\u{2060}", "\u{2061}", "\u{2062}",
        "\u{2063}", "\u{2064}", "\u{2065}", "\u{2066}", "\u{2067}", "\u{2068}", "\u{2069}",
      ].contains(Character(String(scalar)))
  }
  let formulaTriggers: Set<Unicode.Scalar> = ["=", "+", "-", "@"]
  let safe = firstVisible.map(formulaTriggers.contains) == true ? "'\(value)" : value
  return "\"\(safe.replacingOccurrences(of: "\"", with: "\"\""))\""
}

func reportRTFData(_ report: InquiryReport) throws -> Data {
  let document = NSMutableAttributedString(string: "")
  func append(_ text: String, font: NSFont, color: NSColor = .labelColor) {
    document.append(
      NSAttributedString(string: text, attributes: [.font: font, .foregroundColor: color]))
  }
  append(report.query + "\n", font: .systemFont(ofSize: 24, weight: .bold))
  append(report.summary + "\n\n", font: .systemFont(ofSize: 12), color: .secondaryLabelColor)
  if let evidence = report.evidence {
    append(evidence.label + "\n", font: .systemFont(ofSize: 14, weight: .semibold))
    append(evidence.explanation + "\n", font: .systemFont(ofSize: 11))
    append(
      "Coverage: \(evidence.sourceCoverage) · Publishers: \(evidence.publisherDiversity) · Freshness: \(evidence.freshness) · Identity: \(evidence.identityBinding) · Media rights: \(evidence.mediaRights)\n\n",
      font: .monospacedSystemFont(ofSize: 9, weight: .regular), color: .secondaryLabelColor)
  }
  append("Findings\n", font: .systemFont(ofSize: 18, weight: .bold))
  for finding in report.findings {
    append("\n\(finding.title)\n", font: .systemFont(ofSize: 13, weight: .semibold))
    append(finding.body + "\n", font: .systemFont(ofSize: 11))
    append(
      "Support: \(finding.confidence) · Source IDs: \(finding.sourceIds.joined(separator: ", "))\n",
      font: .monospacedSystemFont(ofSize: 9, weight: .regular), color: .secondaryLabelColor)
  }
  if let tables = report.tables, !tables.isEmpty {
    append("\nTables\n", font: .systemFont(ofSize: 18, weight: .bold))
    for table in tables {
      append("\n\(table.title)\n", font: .systemFont(ofSize: 13, weight: .semibold))
      if !table.description.isEmpty {
        append(table.description + "\n", font: .systemFont(ofSize: 11), color: .secondaryLabelColor)
      }

      let columns = inquiryTableDisplayColumns(table)
      if columns.isEmpty {
        append(
          "No columns or rows supplied.\n", font: .systemFont(ofSize: 10),
          color: .secondaryLabelColor)
      } else {
        let headings = columns.map { column in
          column.unit.map { "\(column.label) [\($0)]" } ?? column.label
        }
        append(
          headings.joined(separator: "\t") + "\n",
          font: .monospacedSystemFont(ofSize: 9, weight: .semibold)
        )
        for row in table.rows {
          let values = columns.map { column in
            let value = inquiryTableCell(row, at: column.index)
            return value.isEmpty ? "—" : value
          }
          append(
            values.joined(separator: "\t") + "\n",
            font: .monospacedSystemFont(ofSize: 9, weight: .regular)
          )
        }
      }

      if !table.sourceIds.isEmpty {
        append(
          "Source IDs: \(table.sourceIds.joined(separator: ", "))\n",
          font: .monospacedSystemFont(ofSize: 9, weight: .regular),
          color: .secondaryLabelColor
        )
      }
      if !table.notes.isEmpty {
        append("Notes\n", font: .systemFont(ofSize: 10, weight: .semibold))
        for note in table.notes {
          append("• \(note)\n", font: .systemFont(ofSize: 10), color: .secondaryLabelColor)
        }
      }
    }
  }
  append("\nSources\n", font: .systemFont(ofSize: 18, weight: .bold))
  for source in report.sources {
    append(
      "\n\(source.title) — \(source.publisher)\n", font: .systemFont(ofSize: 11, weight: .semibold))
    append(
      source.url.absoluteString + "\n", font: .monospacedSystemFont(ofSize: 9, weight: .regular))
    append(
      "\(source.quality) · \(source.license ?? "Check source terms")\n",
      font: .systemFont(ofSize: 9), color: .secondaryLabelColor)
  }
  return try document.data(
    from: NSRange(location: 0, length: document.length),
    documentAttributes: [.documentType: NSAttributedString.DocumentType.rtf]
  )
}

private struct PendingExecution {
  let query: String
  var offline: Bool
  let redactSensitive: Bool
  let confirmSensitiveWeb: Bool
  let suppressHistory: Bool
  let privacyFlagged: Bool
}

func shouldStoreRecentQuery(
  privacyFlagged: Bool,
  redactSensitive: Bool,
  confirmSensitiveWeb: Bool
) -> Bool {
  !privacyFlagged && !redactSensitive && !confirmSensitiveWeb
}

func optionalStudyLabel(_ value: String) -> String? {
  let trimmed = value.trimmingCharacters(in: .whitespacesAndNewlines)
  return trimmed.isEmpty ? nil : trimmed
}

func safeStudyPrefix(_ value: String) -> String {
  let normalized = value.lowercased().map { character -> Character in
    character.isASCII && (character.isLetter || character.isNumber) ? character : "-"
  }
  let result = String(normalized)
    .split(separator: "-")
    .filter { !$0.isEmpty }
    .prefix(8)
    .joined(separator: "-")
  return result.isEmpty ? "inquiry-study" : String(result.prefix(60))
}
