import Foundation

enum InquiryPermissionPreferences {
    static let connectorMode = "inquiry.connectorPermissionMode"
}

enum InquiryConnectorPermissionMode: String, CaseIterable, Identifiable, Sendable {
    case askEveryTime
    case automaticPublicWeb
    case offlineOnly

    var id: String { rawValue }

    var title: String {
        switch self {
        case .askEveryTime: "Ask every time"
        case .automaticPublicWeb: "YOLO mode"
        case .offlineOnly: "Always offline"
        }
    }
}

struct InquiryExecutionPlan: Decodable, Sendable, Identifiable {
    var id: String { planId }
    let schemaVersion: String
    let planId: String
    let queryPreview: String
    let intent: InquiryIntentResolution
    let connectors: [InquiryConnectorDisclosure]
    let permissionRequired: Bool
    let automaticEligible: Bool
    let disclosure: String
}

struct InquiryIntentResolution: Decodable, Sendable {
    let kind: String
    let label: String
    let requestedOutputs: [String]
    let clarification: String?
    let rationale: String
}

struct InquiryConnectorDisclosure: Decodable, Sendable, Identifiable {
    let id: String
    let service: String
    let destinations: [String]
    let outboundData: String
    let purpose: String
    let risk: String
    let automaticEligible: Bool
}
