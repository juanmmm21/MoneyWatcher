# 0008 — Looking a merchant up online is opt-in and sends one word

**Status:** accepted

## Context

The assistant only ever sees the movements no rule could classify, and what it fails at is always
the same thing: it does not know what the merchant *is*. It knows Mercadona and Netflix, and it
sends Worten, Consum, Primor, Bricomart and Kiwoko — real Spanish chains, just not famous ones — to
"Otros gastos" with low confidence. Measured on twelve such chains, `gemma3` (4B) got 6 of 12 right
and `phi4` (14B) 8 or 9 depending on the run.

A search engine answers that question in one line. The problem is that asking it is a network call,
and this app's premise (ADR 0001, and the first rule in `CLAUDE.md`) is that financial data never
leaves the machine. The line that rule actually draws is narrower than "no network ever": it is that
the only traffic allowed is traffic the user switched on knowingly, warned about what it implies —
which is exactly how the assistant's own endpoint already works.

The risk is not the HTTP request. It is *what* travels in it. A Spanish statement is full of
concepts that are somebody's name: a Bizum to a friend, a transfer, a payroll line naming the
employer, a rent payment naming the landlord. A merchant token like `mercadona` says nothing about
anyone; `marta lopez` says who somebody paid.

## Decision

The lookup exists, it is **off by default**, and what leaves the machine is one word.

- Only the **learned pattern** travels — the one or two words a rule would be learned from
  (`mercadona`, `leroy merlin`). Never the full concept, the amount, the date, the account or the
  counterparty. The recipient sees a word, not a movement.
- Not even that word when the movement looks like a person: a concept containing `bizum`,
  `transferencia`, `traspaso`, `favor de`, `nómina`, `alquiler` or `hipoteca` is never looked up.
  Tokens with digits (references, card numbers) and tokens shorter than three characters are
  dropped too. `searchable_term` is the single door everything goes through, and it says no by
  default.
- Two sources, neither of which needs an API key: DuckDuckGo's instant answer, and Spanish Wikipedia
  when it says nothing. The endpoints are **hard-coded**. A configurable one would turn a
  categorisation setting into a way of shipping concepts anywhere.
- An answer is only used when it is about a business. DuckDuckGo labels what it found, so a `person`
  or an `athletics event` is thrown away; when the label is missing, the summary itself has to name
  a business (`chain`, `cadena`, `supermarket`, `cooperativa`…), compared word by word so that
  `empresario` — a person — does not pass for `empresa`. A wrong fact is worse than none: "Himilce"
  is a café and the honest answer about the name is a Carthaginian princess.
- Every answer is cached in `brand_lookups`, so each merchant is asked about once and never again.
  What was asked is counted in Settings, and *Olvidar lo consultado* empties it.
- The fact is attached to the line it belongs to, not listed in a block. Measured with `phi4`: the
  same facts as a separate list dropped it from 10 correct to 7, because it read the list as the
  census of merchants that can be recognised and sent everything absent from it to "Otros gastos".

## Consequences

- With the lookup on, `gemma3` goes from 6 to 12 of those twelve chains and `phi4` from 8 to 10.
  The gain is biggest on the smallest model, which is the point: knowing what a shop is is exactly
  what a small model lacks.
- The README can no longer claim the app never talks to the network. It claims what is true: it
  never does unless the user turned something on, and it says what travels when they do.
- Coverage is not complete and will not be. Verdecora and Tiendanimal have no article anywhere; a
  neighbourhood bar never will. Those keep falling back to the heuristics about how Spanish
  businesses are named, which is what already handled them.
- Cached answers age. A chain that is renamed keeps its old description until the cache is emptied.
  That is an acceptable price for asking once instead of every time.
