import AppKit
import Foundation
import Testing

@testable import Inquiry

@Test func reportTableSearchPreservesOrderAndMatchesAcrossCells() {
  let table = reportTableFixture()

  #expect(inquiryTableRows(table, matching: "").map(\.id) == ["row-1", "row-2"])
  #expect(inquiryTableRows(table, matching: "M6 sum").map(\.id) == ["row-1"])
  #expect(inquiryTableRows(table, matching: "1.25").map(\.id) == ["row-2"])
  #expect(inquiryTableRows(table, matching: "ROW-2").map(\.id) == ["row-2"])
  #expect(inquiryTableRows(table, matching: "not present").isEmpty)
}

@Test func reportTableColumnsPreserveExtraAndMissingCellsWithoutInventingMeaning() {
  let table = reportTableFixture()
  let columns = inquiryTableDisplayColumns(table)

  #expect(columns.count == 3)
  #expect(columns[0].key == "designation")
  #expect(columns[1].label == "Pitch")
  #expect(columns[1].unit == "mm")
  #expect(columns[2].key == "unlabeled_3")
  #expect(columns[2].label == "Unlabeled column 3")
  #expect(inquiryTableCell(table.rows[0], at: 2) == "extra observation")
  #expect(inquiryTableCell(table.rows[1], at: 1).isEmpty)
  #expect(inquiryTableCell(table.rows[1], at: 2).isEmpty)
}

@Test func reportCSVIncludesNormalizedTableDataNotesAndFormulaSafety() throws {
  let report = reportWithTableFixture()
  let rows = reportCSVRows(report)

  #expect(rows.allSatisfy { $0.count == 15 })
  let metadata = try #require(rows.first { $0[0] == "table" })
  #expect(metadata[1] == "Common metric threads")
  #expect(metadata[7] == "iso-261")
  #expect(metadata[9] == "metric-threads")
  #expect(metadata[14] == "@verify against current standard | Coarse-pitch subset.")

  let formulaCell = try #require(
    rows.first {
      $0[0] == "table_cell" && $0[10] == "row-1" && $0[11] == "pitch"
    })
  #expect(formulaCell[2] == "=SUM(A1:A2)")
  #expect(formulaCell[12] == "Pitch")
  #expect(formulaCell[13] == "mm")

  let extraCell = try #require(
    rows.first {
      $0[0] == "table_cell" && $0[10] == "row-1" && $0[11] == "unlabeled_3"
    })
  #expect(extraCell[2] == "extra observation")

  let csv = try #require(String(data: reportCSVData(report), encoding: .utf8))
  #expect(csv.contains("\"'=SUM(A1:A2)\""))
  #expect(csv.contains("\"'@verify against current standard | Coarse-pitch subset.\""))
}

@Test func reportRTFIncludesTableHeadingsRowsSourcesAndNotes() throws {
  let data = try reportRTFData(reportWithTableFixture())
  let document = try NSAttributedString(
    data: data,
    options: [.documentType: NSAttributedString.DocumentType.rtf],
    documentAttributes: nil
  )
  let text = document.string

  #expect(text.contains("Tables"))
  #expect(text.contains("Common metric threads"))
  #expect(text.contains("A deliberately narrow lookup table."))
  #expect(text.contains("Pitch [mm]"))
  #expect(text.contains("M6 × 1"))
  #expect(text.contains("=SUM(A1:A2)"))
  #expect(text.contains("extra observation"))
  #expect(text.contains("Source IDs: iso-261"))
  #expect(text.contains("@verify against current standard"))
}

private func reportTableFixture() -> InquiryTableArtifact {
  InquiryTableArtifact(
    id: "metric-threads",
    title: "Common metric threads",
    description: "A deliberately narrow lookup table.",
    columns: [
      InquiryTableColumn(key: "designation", label: "Designation", unit: nil),
      InquiryTableColumn(key: "pitch", label: "Pitch", unit: "mm"),
    ],
    rows: [
      InquiryTableRow(
        id: "row-1",
        cells: ["M6 × 1", "=SUM(A1:A2)", "extra observation"]
      ),
      InquiryTableRow(id: "row-2", cells: ["M8 × 1.25"]),
    ],
    sourceIds: ["iso-261"],
    notes: ["@verify against current standard", "Coarse-pitch subset."]
  )
}

private func reportWithTableFixture() -> InquiryReport {
  InquiryReport(
    schemaVersion: "inquiry.report/v1",
    id: UUID(uuidString: "00000000-0000-4000-8000-000000000118")!,
    createdAt: Date(timeIntervalSince1970: 0),
    query: "common metric threads",
    summary: "Local reference result.",
    confidence: "high",
    evidence: nil,
    findings: [],
    metrics: [],
    tables: [reportTableFixture()],
    sources: [],
    warnings: [],
    run: InquiryRun(
      engineVersion: "test",
      connectorsAttempted: ["local-reference"],
      connectorsSucceeded: ["local-reference"],
      connectorErrors: [],
      networkUsed: false
    )
  )
}
