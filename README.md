# MoneyWatcher

A local-first desktop dashboard for personal finances: drop in your bank statements, let the app
sort them out, and read your money as charts and tables instead of spreadsheet rows.

Your transactions never leave your machine. There is no account, no sync, no telemetry and no
server — the whole app is a single binary and a SQLite file in your user directory.

---

## What it solves

Most people keep their finances in a spreadsheet: one sheet per bank, incomes on one side,
expenses on the other, and a monthly ritual of copying rows out of the bank's CSV export and
tagging them by hand. It works, and it is tedious enough that it eventually stops happening.

MoneyWatcher keeps that mental model — money organised **by bank**, split into **income** and
**expense** — and removes the manual part:

- You import the CSV your bank gives you. The app figures out the delimiter, the encoding, which
  row is the header, which columns hold the date, the description and the amount, and whether the
  file uses `1.234,56` or `1,234.56`.
- Every movement gets categorised by rules that the app learns from your own corrections. Fix
  "MERCADONA" once and every past and future Mercadona lands in Groceries by itself.
- The dashboard is a grid of widgets you arrange yourself: monthly income vs expense, breakdown by
  category, balance per bank, where your money actually goes. Add, resize, drag, remove.

An optional assistant (a local model served by Ollama) can propose categories for the leftovers
that no rule matched. It is off by default, it only ever proposes, and nothing is applied without
your confirmation.

## What makes it interesting

- **Money is never a float.** Every amount is an `i64` of minor units inside a `Money` type, and it
  crosses the Rust ↔ TypeScript boundary as a decimal string. No accumulated rounding error in the
  monthly totals, no `0.30000000000000004` in a balance.
- **Statement parsing that survives real banks.** Preambles before the header, Windows-1252
  encoding, semicolon delimiters, split debit/credit columns, ambiguous `03/04/2026` dates resolved
  once per file instead of row by row, and unreadable rows reported with a reason instead of
  silently dropped.
- **Deterministic first, AI second.** The categorisation engine is a plain, ordered rule evaluator
  that runs offline in microseconds. The LLM is a bolt-on for the residue, isolated behind an
  adapter, and the app is fully functional with the network cable pulled out.
- **Idempotent imports.** Each movement carries a SHA-256 fingerprint of account, date, amount and
  normalised description, so re-importing an overlapping statement adds the new rows and skips the
  ones you already have.

## How it works

```text
   bank CSV ──▶ importer ──▶ preview  ──▶ transactions (SQLite)
                                │              │
                                │              ├──▶ rule engine ──▶ categories
                                │              │         ▲
                                │              │         └── learned from your corrections
                                │              │
                                │              └──▶ analytics ──▶ dashboard widgets
                                │
                                └── nothing is stored until you confirm the preview
```

The Rust core owns every financial decision — parsing, deduplication, rule evaluation, aggregation
and persistence. The React frontend renders what the core returns and never computes money on its
own.

## Architecture

```text
MoneyWatcher/
├── core/                      # moneywatcher-core: the engine, free of any UI dependency
│   ├── migrations/            # versioned SQL schema (0001_initial.sql, …)
│   ├── src/
│   │   ├── domain/            # Money, Account, Transaction, Category, Rule + typed ids
│   │   ├── storage/           # SQLite connection, migrations and repositories
│   │   ├── importer/          # encoding, delimiter, header, column and date detection
│   │   ├── rules/             # rule engine and rule learning
│   │   ├── analytics/         # aggregations that feed the widgets
│   │   └── ai/                # optional assistant (Ollama adapter, prompt, answer parsing)
│   └── tests/                 # integration tests over synthetic statements
├── src-tauri/                 # desktop shell: Tauri commands, state, error mapping
│   └── src/commands/          # one module per area, thin wrappers over the core
├── src/                       # React + TypeScript frontend
│   ├── widgets/               # one component per widget kind + the catalog
│   ├── views/                 # dashboard, transactions, rules, settings
│   ├── components/            # import and account dialogs
│   ├── lib/ipc.ts             # the only place that calls `invoke`
│   └── types/ipc.ts           # the DTO contract mirrored from Rust
└── docs/
    ├── architecture.md
    └── decisions/             # numbered ADRs
```

## Requirements

- **Rust** 1.77 or newer (`rustup` recommended)
- **Node.js** 20 or newer
- **Tauri prerequisites** for your platform (Xcode Command Line Tools on macOS; WebKitGTK and
  `libayatana-appindicator` on Linux; WebView2 on Windows) — see
  [tauri.app/start/prerequisites](https://tauri.app/start/prerequisites/)

SQLite is bundled and compiled from source; you do not need it installed.

## Install and run

```bash
git clone https://github.com/juanmmm21/MoneyWatcher.git
cd MoneyWatcher
npm install

npm run tauri dev      # development window with hot reload
npm run tauri build    # distributable bundle for your platform
```

The database is created on first launch at your platform's application data directory:

| Platform | Path |
| --- | --- |
| macOS | `~/Library/Application Support/com.juanmmm21.moneywatcher/moneywatcher.db` |
| Linux | `~/.local/share/com.juanmmm21.moneywatcher/moneywatcher.db` |
| Windows | `%APPDATA%\com.juanmmm21.moneywatcher\moneywatcher.db` |

Settings → *Your data* shows the exact path in the app. Back it up by copying that file.

## Using it

1. **Create an account per bank** in Settings (bank name, account name, currency, opening balance).
2. **Import a statement**: *Import statement* → pick the account → choose the CSV. The app shows
   what it understood — delimiter, detected columns, the parsed rows and any line it could not
   read — and only writes to the database when you confirm.
3. **Categorise**: rules run automatically after every import. Whatever is left shows up in the
   dashboard as *movimientos sin categorizar*; fix one in the Movimientos table and, with
   *aprender de mis correcciones* enabled, the app writes the rule and applies it to the rest of
   your history.
4. **Build your dashboard**: *+ Añadir widget*, then drag by the widget header and resize from the
   corner. The layout is stored in the database and comes back the next time you open the app.

### Supported statement formats

CSV and tab-separated exports, which is what virtually every bank offers. The importer detects:

| Aspect | Handled |
| --- | --- |
| Delimiter | `;` `,` tab `\|` |
| Encoding | UTF-8 (with or without BOM) and Windows-1252 |
| Header | any row within the first 30, so bank preambles are skipped |
| Date columns | booking date and value date, `dd/mm/yyyy`, `mm/dd/yyyy`, `yyyy-mm-dd`, 2-digit years |
| Amount columns | one signed column, or separate debit/credit columns |
| Amount formats | `1.234,56`, `1,234.56`, `(45,00)`, `12,00-`, `1.234,56 €`, `EUR 12,00` |
| Extra columns | counterparty and running balance when present |

Column names are matched in Spanish and English (`Fecha operación`, `Concepto`, `Importe`, `Cargo`,
`Abono`, `Saldo`, `Date`, `Description`, `Debit`, `Credit`, `Balance`, …).

### The optional assistant

Settings → *Asistente de categorización*. Point it at a local [Ollama](https://ollama.com) instance
(`http://127.0.0.1:11434` by default) and pick a model you have pulled. The app sends only the
description, counterparty and amount of the movements no rule could classify — never account names,
balances or identifiers — and shows the model's proposals for you to accept one by one. If you
point it at a non-local endpoint, the UI warns you explicitly that data would leave your machine.

## Using the core as a library

The engine is a plain Rust crate with no Tauri dependency, so it can be used from a CLI, a script or
your own tooling:

```rust
use moneywatcher_core::domain::AccountId;
use moneywatcher_core::importer::parse_csv;

let bytes = std::fs::read("statement.csv")?;
let preview = parse_csv(&bytes)?;

println!("{} movements, {} unreadable lines", preview.rows.len(), preview.skipped.len());
println!("period total: {}", preview.total_amount().to_decimal_string());

let transactions: Vec<_> = preview
    .rows
    .iter()
    .map(|row| row.to_new_transaction(AccountId(1), None))
    .collect();
```

```rust
use moneywatcher_core::storage::{Database, TransactionFilter};

let database = Database::open(std::path::Path::new("moneywatcher.db"))?;
let totals = database.flow_totals(&TransactionFilter::default())?;

println!("income {} / expense {}", totals.income, totals.expense);
```

## Development

```bash
# Rust core and desktop shell
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all

# Frontend
npm run typecheck
npm run lint
npm test
```

The Rust test suite covers the money type, the SQLite repositories and migrations, the statement
importer (against synthetic statements in `core/tests/fixtures/`), the rule engine and every
dashboard aggregation.

## Troubleshooting

**"could not find a header row with a date, a description and an amount"** — the file is probably
not the movement export but a summary or a PDF-turned-CSV. Export the movement list again, and make
sure the header row contains a date column, a description column and either an amount column or a
debit/credit pair.

**Dates came in with day and month swapped** — the importer resolves the order once per file by
looking for a day above 12 anywhere in the date column. A statement where every date is ambiguous
(all days ≤ 12) falls back to day-first. Undo the import from Settings → *Importaciones recientes*
and re-import with a wider date range so the file contains at least one unambiguous date.

**Re-importing added nothing** — that is the deduplication working: the movements were already
there. Only genuinely new rows are inserted.

**The assistant says "sin respuesta"** — Ollama is not running or is listening elsewhere. Start it
with `ollama serve`, confirm the model is pulled (`ollama list`), and press *Comprobar conexión*.

**Accented characters look wrong** — the file is in an encoding other than UTF-8 or Windows-1252.
Re-export as UTF-8, or open and re-save it as UTF-8 before importing.

## License

[MIT](LICENSE) © juanmmm21
