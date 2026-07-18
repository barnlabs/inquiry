import Foundation

struct InquiryLiveSnapshot: Decodable, Sendable {
    let schemaVersion: String
    let snapshotKind: String
    let executionPlanId: String
    let approvalMode: String
    let retrievedAt: Date
    let latestGeometrySourceTimestamp: Date?
    let sourceAgeSeconds: Int?
    let events: [InquiryLiveEvent]
    let provenance: InquiryLiveProvenance
    let operationalLimits: InquiryLiveOperationalLimits
    let providerRateLimit: InquiryLiveProviderRateLimit?
    let networkUsed: Bool
    let latencyMs: UInt64
    let statusStatement: String
    let warning: String
}

struct InquiryLiveEvent: Decodable, Identifiable, Sendable {
    let id: String
    let title: String
    let description: String?
    let eonetUrl: URL
    let providerStatus: String
    let closedAt: Date?
    let categories: [InquiryLiveCategory]
    let sources: [InquiryLiveSource]
    let geometries: [InquiryLiveGeometry]
    let verificationStatus: String
}

struct InquiryLiveCategory: Decodable, Identifiable, Sendable {
    let id: String
    let title: String
}

struct InquiryLiveSource: Decodable, Identifiable, Sendable {
    let id: String
    let url: URL
    let transport: String?
    let sourceTimestamp: Date?
    let timestampStatement: String
    let automaticallyFetched: Bool
}

struct InquiryLiveGeometry: Decodable, Sendable {
    let sourceTimestamp: Date
    let magnitude: InquiryLiveMagnitude?
    let shape: InquiryLiveGeometryShape
}

struct InquiryLiveMagnitude: Decodable, Sendable {
    let value: Double
    let unit: String
}

struct InquiryLiveGeometryShape: Decodable, Sendable {
    let kind: String
    let position: InquiryLivePosition?
    let rings: [[InquiryLivePosition]]?
}

struct InquiryLivePosition: Decodable, Sendable {
    let longitude: Double
    let latitude: Double
    let altitude: Double?
}

struct InquiryLiveProvenance: Decodable, Sendable {
    let provider: String
    let dataset: String
    let endpoint: String
    let requestScope: String
    let documentationUrl: URL
    let disclaimerUrl: URL
    let curationUrl: URL?
    let verificationStatement: String
    let sourceLinkPolicy: String?
    let operationalNotice: String
    let surveillanceSafeguard: String
}

struct InquiryLiveOperationalLimits: Decodable, Sendable {
    let maxResponseBytes: Int?
    let maxEvents: Int
    let maxCategoriesPerEvent: Int?
    let maxSourcesPerEvent: Int?
    let maxGeometriesPerEvent: Int?
    let maxPolygonRings: Int?
    let maxPositionsPerRing: Int?
    let maxPositionsPerEvent: Int?
    let networkRequestsPerCall: Int
    let automaticRetries: Int
    let backgroundPolling: Bool
    let redirectsFollowed: Bool
    let sourceLinksFetched: Bool
}

struct InquiryLiveProviderRateLimit: Decodable, Sendable {
    let limit: Int?
    let remaining: Int?
    let statement: String
}
