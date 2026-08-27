# 0002 — Money is an integer of minor units, end to end

**Status:** accepted

## Context

Floating point cannot represent most decimal amounts exactly. In a finance app the error is not
theoretical: summing a year of movements in `f64` drifts by cents, and a balance that disagrees with
the bank's statement by one cent destroys trust in every other number on screen.

The boundary between Rust and TypeScript makes it worse: JSON numbers become IEEE-754 doubles, so
even a core that computes exactly would hand the frontend a lossy value.

## Decision

- `Money` wraps an `i64` of minor units (cents) with a fixed scale of 2.
- SQLite stores amounts as `INTEGER`. `REAL` is never used for money.
- Serialisation is a decimal string (`"-1234.56"`), both in the IPC layer and in JSON.
- The TypeScript side formats from that string and never parses it into `number`, except in
  `toChartValue` for drawing charts, where a pixel of error is invisible.

## Consequences

- Every arithmetic operation is exact, and `0.10 × 10` is exactly `1.00`.
- Amount parsing is explicit and testable: `Money::parse_flexible` handles the formats real banks
  emit, including thousands separators, trailing signs and parenthesised negatives.
- Supporting a currency with a different number of decimals (JPY, KWD) would require revisiting the
  fixed scale.
