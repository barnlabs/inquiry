import Foundation
import Testing
@testable import Inquiry

@Test func decodesMinimalReport() throws {
    let data = #"{"schema_version":"inquiry.report/v1","id":"00000000-0000-4000-8000-000000000000","created_at":"2026-07-15T12:00:00Z","query":"test","summary":"ok","confidence":"moderate","findings":[],"metrics":[],"sources":[],"warnings":[],"run":{"engine_version":"0.1.0","connectors_attempted":[],"connectors_succeeded":[],"connector_errors":[],"network_used":false}}"#.data(using: .utf8)!
    let decoder = JSONDecoder()
    decoder.keyDecodingStrategy = .convertFromSnakeCase
    decoder.dateDecodingStrategy = .iso8601
    let report = try decoder.decode(InquiryReport.self, from: data)
    #expect(report.schemaVersion == "inquiry.report/v1")
}

@Test func decodesLiveSnapshotWithFractionalRFC3339AndRateLimitMetadata() throws {
    let data = #"{"schema_version":"inquiry.live.eonet.v1","snapshot_kind":"nasa_eonet_open_natural_events","execution_plan_id":"sha256:fixture","approval_mode":"automatic_public_web","retrieved_at":"2026-07-18T02:22:29.523520Z","latest_geometry_source_timestamp":"2026-07-16T08:04:00Z","source_age_seconds":151709,"events":[{"id":"EONET_1","title":"Fixture wildfire","description":null,"eonet_url":"https://eonet.gsfc.nasa.gov/api/v3/events/EONET_1","provider_status":"open_according_to_eonet","closed_at":null,"categories":[{"id":"wildfires","title":"Wildfires"}],"sources":[{"id":"IRWIN","url":"https://example.test/event","transport":"https","source_timestamp":null,"timestamp_statement":"No per-link timestamp.","automatically_fetched":false}],"geometries":[{"source_timestamp":"2026-07-16T08:04:00Z","magnitude":{"value":2500,"unit":"acres"},"shape":{"kind":"point","position":{"longitude":-120.2,"latitude":45.3,"altitude":null}}}],"verification_status":"provider_curated_not_independently_verified"}],"provenance":{"provider":"NASA EONET","dataset":"EONET v3","endpoint":"https://eonet.gsfc.nasa.gov/api/v3/events?status=open&limit=50","request_scope":"fixed","documentation_url":"https://eonet.gsfc.nasa.gov/docs/v3","disclaimer_url":"https://eonet.gsfc.nasa.gov/what-is-eonet","curation_url":"https://eonet.gsfc.nasa.gov/event-curation","verification_statement":"Provider curated.","source_link_policy":"Links are not fetched.","operational_notice":"One request.","surveillance_safeguard":"Do not track people."},"operational_limits":{"max_response_bytes":1048576,"max_events":50,"max_categories_per_event":8,"max_sources_per_event":16,"max_geometries_per_event":96,"max_polygon_rings":32,"max_positions_per_ring":2048,"max_positions_per_event":4096,"network_requests_per_call":1,"automatic_retries":0,"background_polling":false,"redirects_followed":false,"source_links_fetched":false},"provider_rate_limit":{"limit":60,"remaining":58,"statement":"Copied from response headers."},"network_used":true,"latency_ms":357,"status_statement":"Provider-curated snapshot.","warning":"Not independently verified."}"#.data(using: .utf8)!
    let snapshot = try decodeInquiryJSON(InquiryLiveSnapshot.self, from: data)
    #expect(snapshot.events.first?.geometries.first?.magnitude?.unit == "acres")
    #expect(snapshot.providerRateLimit?.remaining == 58)
    #expect(snapshot.provenance.sourceLinkPolicy == "Links are not fetched.")
}

@Test func decodesStructuredPortraitProvenance() throws {
    let data = #"{"schema_version":"inquiry.report/v1","id":"00000000-0000-4000-8000-000000000000","created_at":"2026-07-17T12:00:00Z","query":"current us president","summary":"ok","confidence":"high","findings":[],"metrics":[],"sources":[{"id":"portrait","title":"portrait","url":"https://commons.wikimedia.org/wiki/File:portrait.jpg","publisher":"Wikimedia Commons","retrieved_at":"2026-07-17T12:00:00Z","published_at":null,"license":"Public domain","source_type":"other","quality":"strong_secondary","provenance":{"dataset_id":"Wikidata Q1 P18 -> portrait.jpg","request_url":"https://commons.wikimedia.org/w/api.php","methodology_url":"https://www.wikidata.org/wiki/Property:P18","observation_period":null,"source_updated_at":null,"content_url":"https://upload.wikimedia.org/portrait.jpg","preview_url":"https://upload.wikimedia.org/thumb/portrait.jpg","file_format":"image/jpeg","file_size_bytes":1000,"width_pixels":1594,"height_pixels":2048,"creator":"Example Photographer","credit":"Example credit","license_url":null,"alt_text":"Official portrait","media_role":"official_portrait","subject_entity_id":"Q1"}}],"warnings":[],"run":{"engine_version":"0.1.0","connectors_attempted":[],"connectors_succeeded":[],"connector_errors":[],"network_used":true}}"#.data(using: .utf8)!
    let decoder = JSONDecoder()
    decoder.keyDecodingStrategy = .convertFromSnakeCase
    decoder.dateDecodingStrategy = .iso8601
    let report = try decoder.decode(InquiryReport.self, from: data)
    #expect(report.sources.first?.provenance.mediaRole == "official_portrait")
    #expect(report.sources.first?.provenance.widthPixels == 1594)
    #expect(report.sources.first?.provenance.creator == "Example Photographer")
}

@Test func portraitLoaderAllowsOnlyExactHTTPSMediaHost() {
    #expect(BoundedMediaDownload.isAllowed(URL(string: "https://upload.wikimedia.org/x.jpg")))
    #expect(!BoundedMediaDownload.isAllowed(URL(string: "http://upload.wikimedia.org/x.jpg")))
    #expect(!BoundedMediaDownload.isAllowed(URL(string: "https://example.com/x.jpg")))
    #expect(!BoundedMediaDownload.isAllowed(URL(string: "https://user@upload.wikimedia.org/x.jpg")))
    #expect(!BoundedMediaDownload.isAllowed(URL(string: "https://upload.wikimedia.org:444/x.jpg")))
}

@Test func portraitRemoteMediaStatesDistinguishSettingsFromOfflineReports() {
    #expect(
        portraitRemoteMediaMode(preferenceEnabled: true, reportUsedNetwork: true)
            == .automatic
    )
    #expect(
        portraitRemoteMediaMode(preferenceEnabled: false, reportUsedNetwork: true)
            == .disabledInSettings
    )
    #expect(
        portraitRemoteMediaMode(preferenceEnabled: true, reportUsedNetwork: false)
            == .offlineReport
    )
    #expect(InquiryPreferences.loadCitedPortraits == "inquiry.loadCitedPortraits")
}

@Test func sensitiveQueriesNeverEnterRecentHistoryByDefault() {
    #expect(!shouldStoreRecentQuery(
        privacyFlagged: true,
        redactSensitive: false,
        confirmSensitiveWeb: false
    ))
    #expect(!shouldStoreRecentQuery(
        privacyFlagged: false,
        redactSensitive: true,
        confirmSensitiveWeb: false
    ))
    #expect(!shouldStoreRecentQuery(
        privacyFlagged: false,
        redactSensitive: false,
        confirmSensitiveWeb: true
    ))
    #expect(shouldStoreRecentQuery(
        privacyFlagged: false,
        redactSensitive: false,
        confirmSensitiveWeb: false
    ))
}

@Test func everySensitiveReviewRequiresFreshOriginalSendAcknowledgement() {
    #expect(shouldResetOriginalSendAcknowledgement())
}

@Test func studyExportPrefixesAreFilesystemSafe() {
    #expect(safeStudyPrefix("BIO 201 / Human Physiology") == "bio-201-human-physiology")
    #expect(safeStudyPrefix("🫀") == "inquiry-study")
    #expect(optionalStudyLabel("   ") == nil)
}

@Test func reportExportsUseSafeNamesAndNeutralizeSpreadsheetFormulas() {
    #expect(safeExportName("../../Common screw sizes", extension: "csv") == "common-screw-sizes.csv")
    #expect(inquiryCSVCell("ordinary") == "\"ordinary\"")
    #expect(inquiryCSVCell("\u{feff}\u{202e}=HYPERLINK(\"https://evil.test\")").hasPrefix("\"'"))
    #expect(inquiryCSVCell("quoted \"value\"") == "\"quoted \"\"value\"\"\"")
}

@Test func findingTagsHideInternalRankingAndUseReadableLabels() {
    #expect(
        visibleFindingTags(["recent-event-candidate", "event-match:3", "rights_checked_media"])
            == ["recent event candidate", "rights checked media"]
    )
}

@MainActor
@Test func newInquiryClearsTransientState() {
    let store = ResearchStore(process: nil)
    store.query = "previous query"
    store.errorMessage = "previous error"
    store.privacyAssessment = InquiryPrivacyAssessment(
        level: "sensitive",
        indicators: ["personal health context"],
        requiresNetworkConfirmation: true,
        redactedQuery: "redacted",
        redactionCount: 1,
        redactedQuerySafeToSend: true,
        guidance: "review"
    )
    store.startNewInquiry()
    #expect(store.query.isEmpty)
    #expect(store.errorMessage == nil)
    #expect(store.privacyAssessment == nil)
    #expect(store.report == nil)
    #expect(!store.isRunning)
    #expect(!store.isRenderingReport)
}

@MainActor
@Test func studySearchReturnsExactMockCitationWithoutNetworkState() async throws {
    let state = MockInquiryState()
    let store = ResearchStore(process: MockInquiryProcess(state: state))
    store.studyIndexSummary = StudyIndexSummary(
        path: "/tmp/mock-study-index.json",
        documentsIndexed: 1,
        segmentsIndexed: 1,
        filesSkipped: 0,
        skipped: [],
        warnings: [],
        applicationNetworkRequests: 0,
        notice: "local"
    )
    store.studyQuery = "primary evidence"
    store.searchStudyIndex()
    for _ in 0..<200 where store.studySearch == nil {
        try await Task.sleep(for: .milliseconds(5))
    }
    #expect(store.studySearch?.results.first?.relativePath == "lecture.md")
    #expect(store.studySearch?.results.first?.contentHash == String(repeating: "a", count: 64))
    #expect(!store.studyIsRunning)
}


private actor MockInquiryState {
    private(set) var oldRunStarted = false
    private(set) var renderStarted = false
    private(set) var approvedPlanID: String?
    private(set) var automaticPublicWeb = false
    private(set) var liveCalls = 0
    private(set) var liveApprovedPlanID: String?
    private(set) var liveAutomaticPublicWeb = false

    func markOldRunStarted() {
        oldRunStarted = true
    }

    func markRenderStarted() {
        renderStarted = true
    }

    func recordPermission(approvedPlanID: String?, automaticPublicWeb: Bool) {
        self.approvedPlanID = approvedPlanID
        self.automaticPublicWeb = automaticPublicWeb
    }

    func recordLivePermission(approvedPlanID: String?, automaticPublicWeb: Bool) {
        liveCalls += 1
        liveApprovedPlanID = approvedPlanID
        liveAutomaticPublicWeb = automaticPublicWeb
    }
}

private enum MockInquiryError: Error {
    case expectedFailure
}

private struct MockInquiryProcess: InquiryProcessing {
    let state: MockInquiryState

    func plan(query: String) async throws -> InquiryExecutionPlan {
        let permissionRequired = query == "permission required" || query == liveWorkspaceQuery
        return InquiryExecutionPlan(
            schemaVersion: "inquiry.execution-plan/v1",
            planId: "sha256:" + String(repeating: "a", count: 64),
            queryPreview: query,
            intent: InquiryIntentResolution(
                kind: "general_research",
                label: "Local mock research",
                requestedOutputs: ["findings"],
                clarification: nil,
                rationale: "test"
            ),
            connectors: permissionRequired ? [
                InquiryConnectorDisclosure(
                    id: "public-example",
                    service: "Public Example",
                    destinations: ["data.example.test"],
                    outboundData: "The exact public query text.",
                    purpose: "Test connector permission.",
                    risk: "public_query",
                    automaticEligible: true
                )
            ] : [],
            permissionRequired: permissionRequired,
            automaticEligible: permissionRequired,
            disclosure: permissionRequired
                ? "One public request is planned."
                : "No public connector request is planned."
        )
    }

    func liveEvents(
        approvedPlanID: String?,
        automaticPublicWeb: Bool
    ) async throws -> InquiryLiveSnapshot {
        await state.recordLivePermission(
            approvedPlanID: approvedPlanID,
            automaticPublicWeb: automaticPublicWeb
        )
        return InquiryLiveSnapshot(
            schemaVersion: "inquiry.live.eonet.v1",
            snapshotKind: "provider_curated_natural_events",
            executionPlanId: "sha256:" + String(repeating: "a", count: 64),
            approvalMode: approvedPlanID == nil ? "automatic_public_web" : "exact_plan_id",
            retrievedAt: Date(timeIntervalSince1970: 1_752_777_600),
            latestGeometrySourceTimestamp: Date(timeIntervalSince1970: 1_752_777_000),
            sourceAgeSeconds: 600,
            events: [],
            provenance: InquiryLiveProvenance(
                provider: "NASA EONET",
                dataset: "EONET v3",
                endpoint: "https://eonet.gsfc.nasa.gov/api/v3/events?status=open&limit=50",
                requestScope: "fixed open events snapshot",
                documentationUrl: URL(string: "https://eonet.gsfc.nasa.gov/docs/v3")!,
                disclaimerUrl: URL(string: "https://eonet.gsfc.nasa.gov/what-is-eonet")!,
                curationUrl: nil,
                verificationStatement: "Provider curated; not independently verified.",
                sourceLinkPolicy: nil,
                operationalNotice: "One request.",
                surveillanceSafeguard: "Do not use for tracking people."
            ),
            operationalLimits: InquiryLiveOperationalLimits(
                maxResponseBytes: nil,
                maxEvents: 50,
                maxCategoriesPerEvent: nil,
                maxSourcesPerEvent: nil,
                maxGeometriesPerEvent: nil,
                maxPolygonRings: nil,
                maxPositionsPerRing: nil,
                maxPositionsPerEvent: nil,
                networkRequestsPerCall: 1,
                automaticRetries: 0,
                backgroundPolling: false,
                redirectsFollowed: false,
                sourceLinksFetched: false
            ),
            providerRateLimit: nil,
            networkUsed: true,
            latencyMs: 10,
            statusStatement: "Provider-curated snapshot.",
            warning: "Not independently verified."
        )
    }

    func research(
        query: String,
        offline: Bool,
        redactSensitive: Bool,
        confirmSensitiveWeb: Bool,
        approvedPlanID: String?,
        automaticPublicWeb: Bool
    ) async throws -> InquiryResearchResult {
        await state.recordPermission(
            approvedPlanID: approvedPlanID,
            automaticPublicWeb: automaticPublicWeb
        )
        if query == "old query" {
            await state.markOldRunStarted()
            try await Task.sleep(for: .seconds(5))
        }
        if query == "failing query" {
            throw MockInquiryError.expectedFailure
        }
        let report = InquiryReport(
            schemaVersion: "inquiry.report/v1",
            id: UUID(),
            createdAt: Date(),
            query: query,
            summary: "mock",
            confidence: "low",
            evidence: nil,
            findings: [],
            metrics: [],
            tables: nil,
            sources: [],
            warnings: [],
            run: InquiryRun(
                engineVersion: "test",
                connectorsAttempted: [],
                connectorsSucceeded: [],
                connectorErrors: [],
                networkUsed: !offline
            )
        )
        return InquiryResearchResult(report: report, data: Data())
    }

    func privacyCheck(query: String) async throws -> InquiryPrivacyAssessment {
        InquiryPrivacyAssessment(
            level: "none",
            indicators: [],
            requiresNetworkConfirmation: false,
            redactedQuery: query,
            redactionCount: 0,
            redactedQuerySafeToSend: false,
            guidance: "review"
        )
    }

    func render(reportData: Data, reportID: UUID) async throws -> URL {
        await state.markRenderStarted()
        try await Task.sleep(for: .seconds(5))
        return URL(fileURLWithPath: "/tmp/unused-\(reportID).html")
    }

    func indexStudy(request: StudyIndexRequest) async throws -> StudyIndexSummary {
        StudyIndexSummary(
            path: request.out,
            documentsIndexed: 1,
            segmentsIndexed: 1,
            filesSkipped: 0,
            skipped: [],
            warnings: [],
            applicationNetworkRequests: 0,
            notice: "local"
        )
    }

    func searchStudy(indexURL: URL, query: String, limit: Int) async throws -> LocalStudySearch {
        LocalStudySearch(
            schemaVersion: "inquiry.study-search/v1",
            query: query,
            course: nil,
            instructor: nil,
            results: [
                LocalStudySearchResult(
                    rank: 1,
                    score: 7.0,
                    relativePath: "lecture.md",
                    locator: "line 9",
                    excerpt: "Exact source excerpt.",
                    contentHash: String(repeating: "a", count: 64),
                    documentHash: String(repeating: "b", count: 64),
                    matchedTerms: ["evidence", "primary"],
                    risks: []
                )
            ],
            warnings: []
        )
    }

    func exportStudyPack(
        indexURL: URL,
        query: String,
        limit: Int,
        outputDirectory: URL,
        prefix: String
    ) async throws -> LocalRecallFiles {
        LocalRecallFiles(
            ankiCsv: outputDirectory.appendingPathComponent("\(prefix)-anki.csv").path,
            quizletTsv: outputDirectory.appendingPathComponent("\(prefix)-quizlet.tsv").path,
            markdown: outputDirectory.appendingPathComponent("\(prefix).md").path,
            json: outputDirectory.appendingPathComponent("\(prefix).json").path
        )
    }
}

@MainActor
@Test func connector_permission_is_required_before_the_first_outbound_run() async throws {
    let state = MockInquiryState()
    let store = ResearchStore(
        process: MockInquiryProcess(state: state),
        permissionMode: { .askEveryTime }
    )
    store.query = "permission required"
    store.research()
    for _ in 0..<200 where store.pendingExecutionPlan == nil {
        try await Task.sleep(for: .milliseconds(5))
    }
    #expect(store.pendingExecutionPlan?.connectors.first?.service == "Public Example")
    #expect(store.report == nil)
    #expect(await state.approvedPlanID == nil)

    store.approvePendingExecutionPlan()
    for _ in 0..<200 where store.report == nil {
        try await Task.sleep(for: .milliseconds(5))
    }
    #expect(await state.approvedPlanID == "sha256:" + String(repeating: "a", count: 64))
    #expect(!(await state.automaticPublicWeb))
}

@MainActor
@Test func yolo_mode_only_uses_an_engine_eligible_public_plan() async throws {
    let state = MockInquiryState()
    let store = ResearchStore(
        process: MockInquiryProcess(state: state),
        permissionMode: { .automaticPublicWeb }
    )
    store.query = "permission required"
    store.research()
    for _ in 0..<200 where store.report == nil {
        try await Task.sleep(for: .milliseconds(5))
    }
    #expect(store.pendingExecutionPlan == nil)
    #expect(await state.approvedPlanID == nil)
    #expect(await state.automaticPublicWeb)
}

@MainActor
@Test func live_workspace_waits_for_one_time_permission_before_fetch_or_map_state() async throws {
    let state = MockInquiryState()
    let store = ResearchStore(
        process: MockInquiryProcess(state: state),
        permissionMode: { .askEveryTime }
    )
    store.refreshLiveEvents()
    for _ in 0..<200 where store.pendingLiveExecutionPlan == nil {
        try await Task.sleep(for: .milliseconds(5))
    }
    #expect(store.pendingLiveExecutionPlan != nil)
    #expect(store.liveSnapshot == nil)
    #expect(await state.liveCalls == 0)

    store.approvePendingLiveExecutionPlan()
    for _ in 0..<200 where store.liveSnapshot == nil {
        try await Task.sleep(for: .milliseconds(5))
    }
    #expect(await state.liveCalls == 1)
    #expect(await state.liveApprovedPlanID == "sha256:" + String(repeating: "a", count: 64))
    #expect(!(await state.liveAutomaticPublicWeb))
}

@MainActor
@Test func live_workspace_yolo_uses_only_the_eligible_fixed_plan() async throws {
    let state = MockInquiryState()
    let store = ResearchStore(
        process: MockInquiryProcess(state: state),
        permissionMode: { .automaticPublicWeb }
    )
    store.refreshLiveEvents()
    for _ in 0..<200 where store.liveSnapshot == nil {
        try await Task.sleep(for: .milliseconds(5))
    }
    #expect(store.pendingLiveExecutionPlan == nil)
    #expect(await state.liveCalls == 1)
    #expect(await state.liveApprovedPlanID == nil)
    #expect(await state.liveAutomaticPublicWeb)
}

@MainActor
@Test func live_workspace_offline_mode_never_calls_the_connector() async throws {
    let state = MockInquiryState()
    let store = ResearchStore(
        process: MockInquiryProcess(state: state),
        permissionMode: { .offlineOnly }
    )
    store.refreshLiveEvents()
    #expect(store.liveSnapshot == nil)
    #expect(store.errorMessage?.contains("did not contact NASA") == true)
    #expect(await state.liveCalls == 0)
}

@MainActor
@Test func cancelled_run_cannot_overwrite_a_new_inquiry() async throws {
    let state = MockInquiryState()
    let store = ResearchStore(process: MockInquiryProcess(state: state))
    store.offline = true
    store.query = "old query"
    store.research()
    while !(await state.oldRunStarted) {
        await Task.yield()
    }

    store.startNewInquiry()
    store.query = "new query"
    store.research()
    for _ in 0..<200 where store.report?.query != "new query" {
        try await Task.sleep(for: .milliseconds(5))
    }

    #expect(store.report?.query == "new query")
    #expect(!store.isRunning)
    try await Task.sleep(for: .milliseconds(50))
    #expect(store.report?.query == "new query")
}

@MainActor
@Test func failed_research_keeps_the_previous_report_visible() async throws {
    let state = MockInquiryState()
    let store = ResearchStore(process: MockInquiryProcess(state: state))
    store.offline = true
    store.query = "working query"
    store.research()
    for _ in 0..<200 where store.report?.query != "working query" {
        try await Task.sleep(for: .milliseconds(5))
    }

    store.query = "failing query"
    store.research()
    for _ in 0..<200 where store.isRunning {
        try await Task.sleep(for: .milliseconds(5))
    }

    #expect(store.report?.query == "working query")
    #expect(store.errorMessage != nil)
}

@MainActor
@Test func rendering_a_report_does_not_replace_research_with_loading_state() async throws {
    let state = MockInquiryState()
    let store = ResearchStore(process: MockInquiryProcess(state: state))
    store.offline = true
    store.query = "renderable query"
    store.research()
    for _ in 0..<200 where store.report == nil {
        try await Task.sleep(for: .milliseconds(5))
    }

    store.openInteractiveReport()
    while !(await state.renderStarted) {
        await Task.yield()
    }

    #expect(store.isRenderingReport)
    #expect(!store.isRunning)
    #expect(store.report?.query == "renderable query")
    store.startNewInquiry()
    #expect(!store.isRenderingReport)
}
