# Architecture

MoneyWatcher is a Tauri 2 desktop application made of three layers with a strict direction of
dependency: the frontend talks to the Tauri shell, the shell talks to the core, and the core talks
to SQLite. Nothing points back up.

```text
┌─────────────────────────────────────────────────────────────┐
│ src/            React + TypeScript                          │
│                 views · widgets · lib/ipc.ts (single door)  │
└───────────────────────────┬─────────────────────────────────┘
                            │ invoke(command, args) — DTOs as JSON
┌───────────────────────────▼─────────────────────────────────┐
│ src-tauri/      Tauri shell                                 │
│                 commands/* · AppState(Mutex<Database>)      │
│                 error mapping to a stable `code` per failure│
└───────────────────────────┬─────────────────────────────────┘
                            │ plain Rust calls
┌───────────────────────────▼─────────────────────────────────┐
│ core/           moneywatcher-core (no UI dependency)        │
│   domain/    Money, Account, Transaction, Category, Rule    │
│   importer/  bytes → ParsedRow, with format detection       │
│   rules/     RuleEngine + learning from corrections         │
│   analytics/ SQL aggregations for the widgets               │
│   ai/        optional Ollama adapter                        │
│   storage/   SQLite: migrations + repositories              │
└─────────────────────────────────────────────────────────────┘
```

## Why a separate core crate

`moneywatcher-core` has no Tauri dependency at all. That buys three things:

- **Fast, isolated tests.** `cargo test -p moneywatcher-core` compiles and runs in seconds without
  building the desktop shell or a webview.
- **CI without system libraries.** The engine is verified on a plain Linux runner; no WebKitGTK.
- **Reuse.** The same engine can back a CLI or a script (see *Using the core as a library* in the
  README) without dragging in a UI framework.

## Data model

Six tables, all created by `core/migrations/0001_initial.sql`:

| Table | Purpose |
| --- | --- |
| `accounts` | one row per account, unique per `(bank, name)`, with its opening balance |
| `categories` | income / expense / transfer buckets; the seeded ones are flagged `is_system` |
| `transactions` | the movements, with `UNIQUE (account_id, fingerprint)` for deduplication |
| `imports` | one row per imported file, so an import can be reverted as a unit |
| `rules` | categorisation rules, ordered by `priority DESC, id ASC` |
| `dashboard_widgets` | widget kind, title, JSON config and grid placement |
| `settings` | key/value app preferences, including the assistant configuration |

Amounts are `INTEGER` columns holding minor units. Dates are `TEXT` in `YYYY-MM-DD`, which sorts
lexicographically and lets month grouping be a plain `substr(booked_on, 1, 7)`.

### Migrations

`MIGRATIONS` in `core/src/storage/mod.rs` is an ordered list of `(version, name, sql)` embedded with
`include_str!`. On open, the database applies every migration above its current version inside a
transaction and records it in `schema_migrations`. Published migrations are never edited — user
databases have already applied them; a change means a new file and a new entry.

## Import pipeline

1. **Decode** — UTF-8 with BOM stripping, falling back to Windows-1252 when the bytes are not valid
   UTF-8 (Spanish banks still export in it).
2. **Delimiter and header** — every candidate delimiter is tried; for each one the first 30 records
   are scanned for a row whose headers map to at least a date, a description and an amount. The
   candidate that maps the most fields wins.
3. **Date order** — resolved once for the whole file by looking for a value above 12 in the date
   column, then applied to every row. Two-digit years are expanded with the POSIX convention.
4. **Rows** — each record becomes a `ParsedRow`, or a `SkippedRow` carrying the reason, which the
   preview dialog shows before anything is written.

Nothing touches the database until the user confirms the preview. On confirmation the rows become
`NewTransaction`s tied to an `imports` row, are inserted in a single SQL transaction with
`INSERT OR IGNORE`, and the rule engine runs over whatever is still uncategorised.

## Categorisation

`RuleEngine` evaluates rules in priority order and returns the first match. A rule can constrain the
description (contains / starts with / ends with / equals), the account, the direction and an
absolute amount range. Matching happens on a normalised string — lowercase, accent- and
punctuation-free, with the counterparty appended — so `COMPRA TARJ. *1234 MERCADONA/VALENCIA` and
`Pago tarjeta, Mercadona` hit the same rule.

When the user corrects a category, `learn_from_correction` extracts the first token of the
description that is neither noise (`compra`, `recibo`, `payment`, `sepa`, …) nor a number, and
creates a `learned` rule at priority 50 — below hand-written rules, which stay at 100. Equivalent
rules are detected before insertion so corrections never pile up duplicates.

The optional assistant sits strictly after this: it only ever sees movements no rule matched, it
receives description, counterparty and amount and nothing else, and its answers are filtered against
the real category list before being shown as proposals.

## Frontend

`src/lib/ipc.ts` is the only module that calls `invoke`, and `src/types/ipc.ts` mirrors the Rust
DTOs. Amounts arrive as decimal strings and are formatted by `src/lib/money.ts` without ever going
through `Number` — the only exception is `toChartValue`, used exclusively to give Recharts a number
to draw, where the difference is not observable.

The dashboard is a `react-grid-layout` grid. Widget kinds live in `src/widgets/registry.tsx`: adding
a kind means adding a catalog entry and a case in `renderWidget`. The layout is persisted on drag or
resize end, not on every frame, and an unknown widget kind renders a placeholder instead of breaking
the whole grid.

## Privacy boundaries

- No telemetry, no analytics, no crash reporting, no auto-update.
- The only outbound request the app can make is to the assistant endpoint the user configures, and
  the UI flags any endpoint that is not loopback.
- The content security policy in `src-tauri/tauri.conf.json` restricts the webview to its own
  origin.
- Statement files are read from the path the user picks through the system dialog; the path is never
  stored, only the file name, so the import list can be shown.
