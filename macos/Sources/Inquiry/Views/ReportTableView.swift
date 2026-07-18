import SwiftUI

struct ReportTablesView: View {
  let tables: [InquiryTableArtifact]

  @State private var searchText = ""
  @FocusState private var searchFieldFocused: Bool

  var body: some View {
    VStack(alignment: .leading, spacing: 14) {
      Text("Tables")
        .font(.title2.bold())
        .accessibilityAddTraits(.isHeader)

      HStack(spacing: 8) {
        TextField("Search table rows", text: $searchText)
          .textFieldStyle(.roundedBorder)
          .focused($searchFieldFocused)
          .accessibilityLabel("Search report table rows")
          .accessibilityHint("Enter one or more terms to filter every report table.")
          .accessibilityIdentifier("inquiry.report.tables.search")
          .frame(maxWidth: 420)

        Button {
          searchFieldFocused = true
        } label: {
          Label("Focus table search", systemImage: "magnifyingglass")
        }
        .labelStyle(.iconOnly)
        .keyboardShortcut("f", modifiers: .command)
        .help("Focus table search (Command-F)")

        if !searchText.isEmpty {
          Button {
            searchText = ""
            searchFieldFocused = true
          } label: {
            Label("Clear table search", systemImage: "xmark.circle.fill")
          }
          .labelStyle(.iconOnly)
          .help("Clear table search")
        }

        Spacer(minLength: 0)

        Text(rowCountLabel)
          .font(.caption.monospacedDigit())
          .foregroundStyle(.secondary)
          .accessibilityLabel(rowCountAccessibilityLabel)
      }

      ForEach(tables) { table in
        ReportRuledTableView(
          table: table,
          rows: inquiryTableRows(table, matching: searchText)
        )
      }
    }
  }

  private var totalRowCount: Int {
    tables.reduce(0) { $0 + $1.rows.count }
  }

  private var visibleRowCount: Int {
    tables.reduce(0) { count, table in
      count + inquiryTableRows(table, matching: searchText).count
    }
  }

  private var rowCountLabel: String {
    searchText.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
      ? "\(totalRowCount) rows"
      : "\(visibleRowCount) of \(totalRowCount) rows"
  }

  private var rowCountAccessibilityLabel: String {
    searchText.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
      ? "\(totalRowCount) table rows"
      : "\(visibleRowCount) of \(totalRowCount) table rows match the search"
  }
}

private struct ReportRuledTableView: View {
  let table: InquiryTableArtifact
  let rows: [InquiryTableRow]

  private let columnWidth: CGFloat = 210

  var body: some View {
    let columns = inquiryTableDisplayColumns(table)

    VStack(alignment: .leading, spacing: 8) {
      HStack(alignment: .firstTextBaseline) {
        Text(table.title)
          .font(.headline)
          .accessibilityAddTraits(.isHeader)
        Spacer()
        Text("\(rows.count) shown")
          .font(.caption.monospacedDigit())
          .foregroundStyle(.secondary)
      }

      if !table.description.isEmpty {
        Text(table.description)
          .font(.subheadline)
          .foregroundStyle(.secondary)
          .textSelection(.enabled)
      }

      if columns.isEmpty {
        Text("No columns or rows were supplied for this table.")
          .font(.callout)
          .foregroundStyle(.secondary)
          .frame(maxWidth: .infinity, alignment: .leading)
          .padding(.vertical, 8)
      } else {
        ScrollView(.horizontal) {
          VStack(alignment: .leading, spacing: 0) {
            tableHeader(columns)
            Divider()

            if rows.isEmpty {
              Text("No rows match the current table search.")
                .font(.callout)
                .foregroundStyle(.secondary)
                .frame(
                  width: max(CGFloat(columns.count) * columnWidth, 320),
                  alignment: .leading
                )
                .padding(.vertical, 14)
                .padding(.horizontal, 10)
            } else {
              ForEach(rows) { row in
                tableRow(row, columns: columns)
                Divider()
              }
            }
          }
          .overlay {
            Rectangle()
              .stroke(.secondary.opacity(0.28), lineWidth: 1)
          }
        }
        .scrollIndicators(.visible)
        .focusable()
        .accessibilityLabel("\(table.title) data table, \(rows.count) visible rows")
        .accessibilityHint("Scroll horizontally to review every column.")
        .accessibilityIdentifier("inquiry.report.table.\(table.id)")
      }

      if !table.sourceIds.isEmpty {
        Text("Source IDs: \(table.sourceIds.joined(separator: ", "))")
          .font(.caption2.monospaced())
          .foregroundStyle(.secondary)
          .textSelection(.enabled)
      }

      if !table.notes.isEmpty {
        VStack(alignment: .leading, spacing: 4) {
          Text("Notes")
            .font(.caption.weight(.semibold))
          ForEach(Array(table.notes.enumerated()), id: \.offset) { _, note in
            Text("• \(note)")
              .font(.caption)
              .foregroundStyle(.secondary)
              .textSelection(.enabled)
          }
        }
        .accessibilityElement(children: .contain)
      }
    }
    .frame(maxWidth: .infinity, alignment: .leading)
    .padding(.vertical, 6)
  }

  private func tableHeader(_ columns: [InquiryTableDisplayColumn]) -> some View {
    HStack(alignment: .top, spacing: 0) {
      ForEach(columns) { column in
        if column.index > 0 { Divider() }
        VStack(alignment: .leading, spacing: 2) {
          Text(column.label)
            .font(.caption.weight(.semibold))
          if let unit = column.unit, !unit.isEmpty {
            Text(unit)
              .font(.caption2)
              .foregroundStyle(.secondary)
          }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(10)
        .frame(width: columnWidth, alignment: .leading)
        .accessibilityElement(children: .combine)
        .accessibilityAddTraits(.isHeader)
      }
    }
    .background(.quaternary)
  }

  private func tableRow(
    _ row: InquiryTableRow,
    columns: [InquiryTableDisplayColumn]
  ) -> some View {
    HStack(alignment: .top, spacing: 0) {
      ForEach(columns) { column in
        if column.index > 0 { Divider() }
        let value = inquiryTableCell(row, at: column.index)
        Text(value.isEmpty ? "—" : value)
          .font(.callout)
          .foregroundStyle(value.isEmpty ? .tertiary : .primary)
          .textSelection(.enabled)
          .fixedSize(horizontal: false, vertical: true)
          .frame(maxWidth: .infinity, alignment: .topLeading)
          .padding(10)
          .frame(width: columnWidth, alignment: .topLeading)
          .accessibilityLabel(column.label)
          .accessibilityValue(value.isEmpty ? "Not supplied" : value)
      }
    }
    .accessibilityElement(children: .contain)
    .accessibilityLabel("Table row \(row.id)")
  }
}
