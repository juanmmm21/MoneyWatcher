# 0005 — Movements are recorded, balances are not

**Status:** accepted

## Context

The app started out storing an opening balance per account and deriving a current balance from it
plus every movement. That number is only right under conditions the app cannot enforce: the user
has to type the opening balance correctly, and then import every statement, in order, with no gaps.
Miss one month and the balance shown next to your bank's name is quietly wrong — worse than absent,
because it looks authoritative.

The value the app actually adds is elsewhere: what you spend, on what, and how it moves month to
month. None of that needs a balance.

## Decision

Accounts have no balance and the app never derives one. What is stored is the list of movements.

The balance a statement reports for each row (`transactions.balance_after`) is still read and kept,
but it is never presented as the account's balance. Its only job is verification: between two
consecutive rows of a statement, the jump in balance has to be exactly the amount in between. If it
is not, the file was misread, and that check is what caught a bank charging fees in a column the
importer was ignoring.

Where the dashboard used to show a balance per bank it now shows the period's net — income minus
expense over the range being looked at — which is a statement about the movements, not a claim
about the bank.

## Consequences

- No number in the app can be wrong because of an import the user forgot to do.
- Creating an account is three fields, and none of them requires looking anything up in the bank.
- The app cannot answer "how much do I have"; the bank's own app answers that, correctly, for free.
- Migration `0005_drop_opening_balance.sql` drops the column. Anything that needs a running total
  computes it from the movements it is showing.
