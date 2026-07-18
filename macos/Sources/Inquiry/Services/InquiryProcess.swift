import Foundation

func makeInquiryJSONDecoder() -> JSONDecoder {
    let decoder = JSONDecoder()
    decoder.keyDecodingStrategy = .convertFromSnakeCase
    decoder.dateDecodingStrategy = .custom { decoder in
        let container = try decoder.singleValueContainer()
        let value = try container.decode(String.self)

        let fractional = ISO8601DateFormatter()
        fractional.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
        if let date = fractional.date(from: value) {
            return date
        }

        let wholeSeconds = ISO8601DateFormatter()
        wholeSeconds.formatOptions = [.withInternetDateTime]
        if let date = wholeSeconds.date(from: value) {
            return date
        }

        throw DecodingError.dataCorruptedError(
            in: container,
            debugDescription: "Expected an RFC 3339 timestamp, with optional fractional seconds."
        )
    }
    return decoder
}

private func inquiryDecodingFailure(_ error: Error) -> String {
    guard let decodingError = error as? DecodingError else {
        return error.localizedDescription
    }
    let context: DecodingError.Context
    let kind: String
    switch decodingError {
    case .typeMismatch(_, let value):
        kind = "type mismatch"
        context = value
    case .valueNotFound(_, let value):
        kind = "missing value"
        context = value
    case .keyNotFound(let key, let value):
        kind = "missing key \(key.stringValue)"
        context = value
    case .dataCorrupted(let value):
        kind = "invalid value"
        context = value
    @unknown default:
        return decodingError.localizedDescription
    }
    let path = context.codingPath.map(\.stringValue).joined(separator: ".")
    return path.isEmpty
        ? "\(kind): \(context.debugDescription)"
        : "\(kind) at \(path): \(context.debugDescription)"
}

enum InquiryProcessError: LocalizedError {
    case binaryMissing
    case failed(String)
    case invalidOutput(String)

    var errorDescription: String? {
        switch self {
        case .binaryMissing:
            "The Inquiry research engine is missing from the app bundle. Rebuild with script/build_and_run.sh."
        case .failed(let message):
            "Inquiry could not complete the research: \(message)"
        case .invalidOutput(let message):
            "Inquiry returned an unreadable report: \(message)"
        }
    }
}

func decodeInquiryJSON<T: Decodable>(_ type: T.Type, from data: Data) throws -> T {
    do {
        return try makeInquiryJSONDecoder().decode(type, from: data)
    } catch {
        throw InquiryProcessError.invalidOutput(inquiryDecodingFailure(error))
    }
}

struct InquiryResearchResult: Sendable {
    let report: InquiryReport
    let data: Data
}

protocol InquiryProcessing: Sendable {
    func plan(query: String) async throws -> InquiryExecutionPlan
    func research(
        query: String,
        offline: Bool,
        redactSensitive: Bool,
        confirmSensitiveWeb: Bool,
        approvedPlanID: String?,
        automaticPublicWeb: Bool
    ) async throws -> InquiryResearchResult
    func privacyCheck(query: String) async throws -> InquiryPrivacyAssessment
    func liveEvents(
        approvedPlanID: String?,
        automaticPublicWeb: Bool
    ) async throws -> InquiryLiveSnapshot
    func render(reportData: Data, reportID: UUID) async throws -> URL
    func discardRenderedReport(at url: URL) async
    func cleanupRenderedReports() async
    func indexStudy(request: StudyIndexRequest) async throws -> StudyIndexSummary
    func searchStudy(indexURL: URL, query: String, limit: Int) async throws -> LocalStudySearch
    func exportStudyPack(
        indexURL: URL,
        query: String,
        limit: Int,
        outputDirectory: URL,
        prefix: String
    ) async throws -> LocalRecallFiles
}

extension InquiryProcessing {
    func discardRenderedReport(at _: URL) async {}
    func cleanupRenderedReports() async {}
    func liveEvents(
        approvedPlanID _: String?,
        automaticPublicWeb _: Bool
    ) async throws -> InquiryLiveSnapshot {
        throw InquiryProcessError.failed("This Inquiry engine does not provide the Live workspace")
    }
}

struct InquiryProcess: InquiryProcessing, Sendable {
    private let binaryURL: URL

    init(binaryURL: URL? = nil) throws {
        if let binaryURL {
            self.binaryURL = binaryURL
            return
        }
        guard let resourceURL = Bundle.main.resourceURL else { throw InquiryProcessError.binaryMissing }
        let bundled = resourceURL.appendingPathComponent("inquiry", isDirectory: false)
        guard FileManager.default.isExecutableFile(atPath: bundled.path) else { throw InquiryProcessError.binaryMissing }
        self.binaryURL = bundled
    }

    func research(
        query: String,
        offline: Bool,
        redactSensitive: Bool = false,
        confirmSensitiveWeb: Bool = false,
        approvedPlanID: String? = nil,
        automaticPublicWeb: Bool = false
    ) async throws -> InquiryResearchResult {
        var arguments = ["research", "--stdin", "--format", "json", "--limit", "12"]
        if offline { arguments.append("--offline") }
        if redactSensitive { arguments.append("--redact-sensitive") }
        if confirmSensitiveWeb { arguments.append("--confirm-sensitive-web") }
        if let approvedPlanID { arguments.append(contentsOf: ["--approved-plan", approvedPlanID]) }
        if automaticPublicWeb { arguments.append("--automatic-public-web") }
        let data = try await run(arguments: arguments, standardInput: Data(query.utf8))
        return InquiryResearchResult(
            report: try decodeInquiryJSON(InquiryReport.self, from: data),
            data: data
        )
    }

    func plan(query: String) async throws -> InquiryExecutionPlan {
        let data = try await run(
            arguments: ["plan", "--stdin"],
            standardInput: Data(query.utf8)
        )
        return try decode(InquiryExecutionPlan.self, from: data)
    }

    func privacyCheck(query: String) async throws -> InquiryPrivacyAssessment {
        let data = try await run(
            arguments: ["privacy-check", "--stdin"],
            standardInput: Data(query.utf8)
        )
        return try decodeInquiryJSON(InquiryPrivacyAssessment.self, from: data)
    }

    func liveEvents(
        approvedPlanID: String?,
        automaticPublicWeb: Bool
    ) async throws -> InquiryLiveSnapshot {
        var arguments = ["live-events"]
        if let approvedPlanID {
            arguments.append(contentsOf: ["--approved-plan", approvedPlanID])
        }
        if automaticPublicWeb {
            arguments.append("--automatic-public-web")
        }
        let data = try await run(arguments: arguments)
        return try decode(InquiryLiveSnapshot.self, from: data)
    }

    func render(reportData: Data, reportID: UUID) async throws -> URL {
        let directory = renderedReportsDirectory
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        let directoryValues = try directory.resourceValues(forKeys: [.isDirectoryKey, .isSymbolicLinkKey])
        guard directoryValues.isDirectory == true, directoryValues.isSymbolicLink != true else {
            throw InquiryProcessError.invalidOutput("Private report storage is not a real directory")
        }
        try FileManager.default.setAttributes([.posixPermissions: 0o700], ofItemAtPath: directory.path)
        let output = directory.appendingPathComponent("\(reportID.uuidString.lowercased()).html")
        if FileManager.default.fileExists(atPath: output.path) {
            try FileManager.default.removeItem(at: output)
        }
        do {
            _ = try await run(
                arguments: ["render-report", "--out", output.path],
                standardInput: reportData
            )
        } catch {
            try? FileManager.default.removeItem(at: output)
            throw error
        }
        guard FileManager.default.fileExists(atPath: output.path) else { throw InquiryProcessError.invalidOutput("HTML file was not created") }
        try FileManager.default.setAttributes([.posixPermissions: 0o600], ofItemAtPath: output.path)
        let values = try output.resourceValues(forKeys: [.isRegularFileKey, .isSymbolicLinkKey])
        guard values.isRegularFile == true, values.isSymbolicLink != true else {
            try? FileManager.default.removeItem(at: output)
            throw InquiryProcessError.invalidOutput("HTML output was not a private regular file")
        }
        return output
    }

    func discardRenderedReport(at url: URL) async {
        let candidate = url.standardizedFileURL
        guard candidate.deletingLastPathComponent() == renderedReportsDirectory.standardizedFileURL,
              candidate.pathExtension.lowercased() == "html" else { return }
        try? FileManager.default.removeItem(at: candidate)
    }

    func cleanupRenderedReports() async {
        let directory = renderedReportsDirectory
        guard let directoryValues = try? directory.resourceValues(
            forKeys: [.isDirectoryKey, .isSymbolicLinkKey]
        ), directoryValues.isDirectory == true, directoryValues.isSymbolicLink != true else { return }
        guard let entries = try? FileManager.default.contentsOfDirectory(
            at: directory,
            includingPropertiesForKeys: [.isRegularFileKey, .isSymbolicLinkKey],
            options: [.skipsHiddenFiles]
        ) else { return }
        for entry in entries where entry.pathExtension.lowercased() == "html" {
            let values = try? entry.resourceValues(forKeys: [.isRegularFileKey, .isSymbolicLinkKey])
            if values?.isRegularFile == true, values?.isSymbolicLink != true {
                try? FileManager.default.removeItem(at: entry)
            }
        }
        try? FileManager.default.setAttributes([.posixPermissions: 0o700], ofItemAtPath: directory.path)
    }

    private var renderedReportsDirectory: URL {
        FileManager.default.temporaryDirectory
            .appendingPathComponent("BarnLabs-Inquiry", isDirectory: true)
    }

    func indexStudy(request: StudyIndexRequest) async throws -> StudyIndexSummary {
        let encoder = JSONEncoder()
        encoder.keyEncodingStrategy = .convertToSnakeCase
        let requestData = try encoder.encode(request)
        let data = try await run(
            arguments: ["study-index", "--request-stdin"],
            standardInput: requestData
        )
        return try decode(StudyIndexSummary.self, from: data)
    }

    func searchStudy(indexURL: URL, query: String, limit: Int) async throws -> LocalStudySearch {
        let data = try await run(
            arguments: [
                "study-search",
                indexURL.path,
                "--stdin",
                "--json",
                "--limit",
                String(min(max(limit, 1), 20))
            ],
            standardInput: Data(query.utf8)
        )
        return try decode(LocalStudySearch.self, from: data)
    }

    func exportStudyPack(
        indexURL: URL,
        query: String,
        limit: Int,
        outputDirectory: URL,
        prefix: String
    ) async throws -> LocalRecallFiles {
        let data = try await run(
            arguments: [
                "study-local-pack",
                indexURL.path,
                "--stdin",
                "--limit",
                String(min(max(limit, 1), 30)),
                "--out-dir",
                outputDirectory.path,
                "--prefix",
                prefix
            ],
            standardInput: Data(query.utf8)
        )
        return try decode(LocalRecallFiles.self, from: data)
    }

    private func decode<T: Decodable>(_ type: T.Type, from data: Data) throws -> T {
        try decodeInquiryJSON(type, from: data)
    }

    private func run(arguments: [String], standardInput: Data? = nil) async throws -> Data {
        let binaryURL = self.binaryURL
        let controller = InquiryProcessController()
        return try await withTaskCancellationHandler {
            try await Task.detached(priority: .userInitiated) {
            let process = Process()
            controller.install(process)
            defer { controller.clear(process) }
            let output = Pipe()
            let errors = Pipe()
            let input = Pipe()
            process.executableURL = binaryURL
            process.arguments = arguments
            process.standardOutput = output
            process.standardError = errors
            if standardInput != nil { process.standardInput = input }
            do { try process.run() }
            catch { throw InquiryProcessError.failed(error.localizedDescription) }
            if controller.isCancelled {
                process.terminate()
            }
            if let standardInput {
                input.fileHandleForWriting.write(standardInput)
                try? input.fileHandleForWriting.close()
            }
            let outputTask = Task.detached { output.fileHandleForReading.readDataToEndOfFile() }
            let errorTask = Task.detached { errors.fileHandleForReading.readDataToEndOfFile() }
            let clock = ContinuousClock()
            let deadline = clock.now + .seconds(60)
            while process.isRunning {
                if controller.isCancelled {
                    process.terminate()
                    throw CancellationError()
                }
                if clock.now >= deadline {
                    process.terminate()
                    throw InquiryProcessError.failed("engine exceeded the 60-second limit")
                }
                try await Task.sleep(for: .milliseconds(50))
            }
            let data = await outputTask.value
            let errorData = await errorTask.value
            if controller.isCancelled {
                throw CancellationError()
            }
            guard process.terminationStatus == 0 else {
                let message = String(data: errorData, encoding: .utf8)?.trimmingCharacters(in: .whitespacesAndNewlines)
                throw InquiryProcessError.failed(message?.isEmpty == false ? message! : "engine exited with status \(process.terminationStatus)")
            }
            return data
            }.value
        } onCancel: {
            controller.cancel()
        }
    }
}

private final class InquiryProcessController: @unchecked Sendable {
    private let lock = NSLock()
    private var process: Process?
    private var cancelled = false

    var isCancelled: Bool {
        lock.withLock { cancelled }
    }

    func install(_ process: Process) {
        lock.withLock { self.process = process }
    }

    func clear(_ process: Process) {
        lock.withLock {
            if self.process === process {
                self.process = nil
            }
        }
    }

    func cancel() {
        let running = lock.withLock { () -> Process? in
            cancelled = true
            return process
        }
        if running?.isRunning == true {
            running?.terminate()
        }
    }
}
