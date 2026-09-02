# 0006 — One currency: the euro

**Status:** accepted

## Context

Accounts used to carry an ISO 4217 code, which turned out to be a promise the rest of the app did
not keep: the aggregations summed every account together regardless of currency, so a dashboard
with accounts in two currencies showed a total that meant nothing, labelled with the currency of
whichever account happened to be created first.

Making it honest is not hard — filter every aggregation by currency and show one at a time — but
useful multi-currency support is a different, larger thing: it needs exchange rates, which need a
network call and a date, and both are exactly what this app avoids.

## Decision

MoneyWatcher works in euros. Accounts have no currency field, amounts are formatted with `€`, and
no query branches on currency.

## Consequences

- Every total in the app is a sum of comparable numbers, by construction rather than by convention.
- A statement in another currency can still be imported into an account, but the app will label the
  amounts in euros, so it should not be.
- Supporting more currencies later means reintroducing the column, filtering the aggregations by it
  and revisiting the fixed two-decimal scale of `Money` (see ADR 0002) — a currency with zero or
  three decimals does not fit today's type.
- Migration `0004_drop_account_currency.sql` drops the column.
