# 0004 — Statement formats are detected, not configured

**Status:** accepted

## Context

Bank CSV exports have no standard. Within Spain alone you find semicolons, Windows-1252 encoding,
four preamble rows before the header, `Fecha operación` next to `Fecha valor`, amounts as
`-1.234,56` or split across `Cargo` and `Abono` columns, and trailing total rows that are not
movements. Asking the user to map columns by hand for every bank turns a five-second task into a
form.

## Decision

The importer detects everything it can and reports what it could not:

- Delimiter chosen by scoring each candidate on how many fields its header row maps.
- Encoding: UTF-8 with BOM stripping, falling back to Windows-1252.
- Header found anywhere in the first 30 records, so preambles are skipped.
- Columns matched against Spanish and English keyword lists, most specific first, each column
  consumed once so two fields never read the same cell.
- Date order resolved once per file, never per row.

Rows that cannot be read become `SkippedRow` entries with a human-readable reason, shown in the
preview dialog. Nothing is written to the database until the user confirms that preview.

## Consequences

- A new bank usually works with no configuration at all.
- A bank that does not can be supported by extending the keyword lists in `importer/mapping.rs`,
  which is a one-line change plus a fixture.
- The user always sees what was understood before it is stored, and a whole import can be reverted
  as a unit from Settings.
