import Foundation

struct InquiryReport: Decodable, Sendable {
  let schemaVersion: String
  let id: UUID
  let createdAt: Date
  let query: String
  let summary: String
  let confidence: String
  let evidence: InquiryEvidenceAssessment?
  let findings: [InquiryFinding]
  let metrics: [InquiryMetric]
  let tables: [InquiryTableArtifact]?
  let sources: [InquirySource]
  let warnings: [String]
  let run: InquiryRun
}

struct InquiryTableArtifact: Decodable, Identifiable, Sendable {
  let id: String
  let title: String
  let description: String
  let columns: [InquiryTableColumn]
  let rows: [InquiryTableRow]
  let sourceIds: [String]
  let notes: [String]
}

struct InquiryTableColumn: Decodable, Sendable {
  let key: String
  let label: String
  let unit: String?
}

struct InquiryTableRow: Decodable, Identifiable, Sendable {
  let id: String
  let cells: [String]
}

struct InquiryTableDisplayColumn: Identifiable, Equatable, Sendable {
  let id: String
  let index: Int
  let key: String
  let label: String
  let unit: String?
}

func inquiryTableDisplayColumns(_ table: InquiryTableArtifact) -> [InquiryTableDisplayColumn] {
  let widestRow = table.rows.map(\.cells.count).max() ?? 0
  let count = max(table.columns.count, widestRow)
  return (0..<count).map { index in
    if table.columns.indices.contains(index) {
      let column = table.columns[index]
      return InquiryTableDisplayColumn(
        id: "\(index):\(column.key)",
        index: index,
        key: column.key,
        label: column.label,
        unit: column.unit
      )
    }
    let number = index + 1
    return InquiryTableDisplayColumn(
      id: "\(index):unlabeled",
      index: index,
      key: "unlabeled_\(number)",
      label: "Unlabeled column \(number)",
      unit: nil
    )
  }
}

func inquiryTableCell(_ row: InquiryTableRow, at columnIndex: Int) -> String {
  row.cells.indices.contains(columnIndex) ? row.cells[columnIndex] : ""
}

func inquiryTableRows(
  _ table: InquiryTableArtifact,
  matching query: String
) -> [InquiryTableRow] {
  let normalizedQuery = inquiryTableSearchText(query)
  let terms = inquiryTableSearchTerms(query)
  guard !terms.isEmpty else { return table.rows }

  return table.rows.filter { row in
    if inquiryTableSearchText(row.id) == normalizedQuery { return true }
    let searchable = inquiryTableSearchText(row.cells.joined(separator: " "))
    return terms.allSatisfy(searchable.contains)
  }
}

private func inquiryTableSearchTerms(_ value: String) -> [String] {
  inquiryTableSearchText(value)
    .split(separator: " ")
    .map(String.init)
}

private func inquiryTableSearchText(_ value: String) -> String {
  let folded = value.folding(
    options: [.caseInsensitive, .diacriticInsensitive],
    locale: Locale(identifier: "en_US_POSIX")
  )
  return folded.unicodeScalars.map { scalar in
    CharacterSet.alphanumerics.contains(scalar) || scalar == "." ? Character(String(scalar)) : " "
  }
  .reduce(into: "") { result, character in
    if character == " ", result.last == " " { return }
    result.append(character)
  }
  .trimmingCharacters(in: .whitespacesAndNewlines)
}

struct InquiryEvidenceAssessment: Decodable, Sendable {
  let status: String
  let label: String
  let explanation: String
  let sourceCoverage: String
  let publisherDiversity: String
  let freshness: String
  let identityBinding: String
  let mediaRights: String
}

struct InquiryRun: Decodable, Sendable {
  let engineVersion: String
  let connectorsAttempted: [String]
  let connectorsSucceeded: [String]
  let connectorErrors: [String]
  let networkUsed: Bool
}

struct InquiryFinding: Decodable, Identifiable, Sendable {
  let id: String
  let title: String
  let body: String
  let facet: String
  let confidence: String
  let sourceIds: [String]
  let contentTrust: String
  let tags: [String]
}

struct InquiryMetric: Decodable, Identifiable, Sendable {
  var id: String { "\(label)-\(period ?? "")" }
  let label: String
  let value: Double
  let displayValue: String
  let unit: String
  let facet: String
  let sourceIds: [String]
  let period: String?
}

struct InquirySource: Decodable, Identifiable, Sendable {
  let id: String
  let title: String
  let url: URL
  let publisher: String
  let retrievedAt: Date
  let publishedAt: String?
  let license: String?
  let sourceType: String
  let quality: String
  let provenance: InquiryProvenance
}

struct InquiryProvenance: Decodable, Sendable {
  let datasetId: String?
  let requestUrl: String?
  let methodologyUrl: String?
  let observationPeriod: String?
  let sourceUpdatedAt: String?
  let contentUrl: URL?
  let previewUrl: URL?
  let fileFormat: String?
  let fileSizeBytes: UInt64?
  let widthPixels: UInt64?
  let heightPixels: UInt64?
  let creator: String?
  let credit: String?
  let licenseUrl: URL?
  let altText: String?
  let mediaRole: String?
  let subjectEntityId: String?
}
