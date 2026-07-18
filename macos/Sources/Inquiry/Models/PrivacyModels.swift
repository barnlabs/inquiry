import Foundation

struct InquiryPrivacyAssessment: Decodable, Sendable {
    let level: String
    let indicators: [String]
    let requiresNetworkConfirmation: Bool
    let redactedQuery: String
    let redactionCount: Int
    let redactedQuerySafeToSend: Bool
    let guidance: String
}
