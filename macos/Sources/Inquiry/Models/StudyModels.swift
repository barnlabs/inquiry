import Foundation

struct StudyIndexRequest: Encodable, Sendable {
    let directory: String
    let out: String
    let course: String?
    let instructor: String?
    let includeSpeakerNotes: Bool
}

struct StudyIndexSummary: Decodable, Sendable {
    let path: String
    let documentsIndexed: Int
    let segmentsIndexed: Int
    let filesSkipped: Int
    let skipped: [StudySkipped]
    let warnings: [String]
    let applicationNetworkRequests: Int
    let notice: String
}

struct StudySkipped: Decodable, Identifiable, Sendable {
    var id: String { "\(relativePath):\(reason)" }
    let relativePath: String
    let reason: String
}

struct LocalStudySearch: Decodable, Sendable {
    let schemaVersion: String
    let query: String
    let course: String?
    let instructor: String?
    let results: [LocalStudySearchResult]
    let warnings: [String]
}

struct LocalStudySearchResult: Decodable, Identifiable, Sendable {
    var id: String { "\(relativePath):\(locator):\(contentHash)" }
    let rank: Int
    let score: Double
    let relativePath: String
    let locator: String
    let excerpt: String
    let contentHash: String
    let documentHash: String
    let matchedTerms: [String]
    let risks: [String]
}

struct LocalRecallFiles: Decodable, Sendable {
    let ankiCsv: String
    let quizletTsv: String
    let markdown: String
    let json: String
}
