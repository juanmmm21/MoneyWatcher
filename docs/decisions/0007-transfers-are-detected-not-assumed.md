# 0007 — Transfers between your own accounts are detected, never assumed

**Status:** accepted

## Context

Moving 300 € from a current account to a savings account produces two movements: a charge in one
account and a deposit in the other. Counted separately they are a 300 € expense and a 300 € income,
and the dashboard says so — inflating both columns with money that never left the user's hands,
flattening the savings rate, and putting the receiving bank in the list of places where money is
spent.

Measured on seven real statements (3,802 movements across five banks), 161 pairs were transfers.
They accounted for 42,621 € of "income" and the same amount of "expense" — more than a third of
both totals. Every conclusion the dashboard offered about that year was distorted by them.

Nothing in a bank statement says "this is a transfer". The only signal available is the shape of
the pair: the same amount with opposite signs, in two different accounts, a day or two apart. That
shape recognises a transfer, but it also recognises a coincidence — an invoice paid on the same day
a refund of the same amount arrives.

## Decision

The app pairs the two sides and stores the link, but it never decides on its own that the pairing
is right, and it never changes what a movement is.

- Detection is **off until the user turns it on**, in Settings. Turning it on runs it over the whole
  history at once; from then on it also runs after each import, because a new statement usually
  brings the other half of a transfer that was already stored.
- Only when it is on do the aggregations exclude linked movements. The setting can be switched off
  at any time and the totals go back to what they were: the links stay, they simply stop applying.
- Matching is exact: same amount to the cent, opposite signs, different accounts, at most two days
  apart. Each movement belongs to at most one pair, and when several candidates fit, the closest in
  time wins — ties broken by id, so the result never depends on the order rows came back in.
- Every pair is listed in Settings and can be dismissed. A dismissed pair is **not deleted**: it is
  marked. Deleting it would free both movements and the next detection would propose exactly the
  same pair again, which is the app arguing with the user.
- The movement list keeps showing transfers, tagged. They are in the bank's statement; hiding them
  from the one view that mirrors the statement would confuse more than it helps.

## Consequences

- The dashboard can be read as "money in and out of my hands" instead of "money in and out of each
  account", which is the question the user was actually asking.
- A wrong pair is visible and reversible in one click, and stays reversed.
- Two days is a compromise: transfers between different banks usually land the next day, and a
  wider window starts matching coincidences — in a year of movements, two equal amounts with
  opposite signs in the same week are easy to find.
- This does not contradict ADR 0005. No balance is invented; two movements the app already had are
  marked as the two faces of one transfer.
