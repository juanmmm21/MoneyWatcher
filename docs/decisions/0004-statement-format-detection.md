# 0004 — Statement formats are detected, not configured

**Status:** accepted

## Context

Bank exports have no standard. Within Spain alone you find semicolons, Windows-1252 encoding, four
preamble rows before the header, `Fecha operación` next to `Fecha valor`, amounts as `-1.234,56` or
split across `Cargo` and `Abono` columns, and trailing total rows that are not movements. Several
banks do not offer CSV at all, only an Excel workbook with a cover sheet in front of the movements.
Asking the user to map columns by hand for every bank turns a five-second task into a form.

## Decision

The importer detects everything it can and reports what it could not:

- File type decided by the leading bytes, not the extension: a ZIP or a Compound File is a
  spreadsheet, anything else is text.
- Delimiter — and, in a workbook, sheet — chosen by scoring each candidate on how many fields its
  header row maps.
- Encoding: UTF-8 with BOM stripping, falling back to Windows-1252.
- Header found anywhere in the first 500 rows, so preambles and cover sheets are skipped.
- Columns matched against Spanish and English keyword lists, most specific first, each column
  consumed once so two fields never read the same cell.
- Date order resolved once per file, never per row.
- Whether a fee column is charged on top of the amount decided by whichever reading matches the
  balance the statement reports, instead of by a list of banks.

Rows that cannot be read become `SkippedRow` entries with a human-readable reason, shown in the
preview dialog. Nothing is written to the database until the user confirms that preview.

## Consequences

- A new bank usually works with no configuration at all.
- A bank that does not can be supported by extending the keyword lists in `importer/mapping.rs`,
  which is a one-line change plus a fixture.
- Detection can be wrong in ways a user would not notice, so it is verified rather than trusted:
  when the statement reports a running balance, every amount is checked against it.
- The user always sees what was understood before it is stored, and a whole import can be reverted
  as a unit from Settings.
