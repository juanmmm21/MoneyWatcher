# 0003 — Deterministic rules first, AI as an optional accelerator

**Status:** accepted

## Context

"AI that organises your statements" is the obvious pitch, but a model as the primary classifier has
three problems here: it makes the app useless offline, it is unpredictable (the same movement can be
classified differently on two runs), and pointing it at a hosted provider would send the user's
transaction history to a third party — exactly what this app promises not to do.

## Decision

Categorisation is a rule engine. Rules are ordered by priority, evaluated top to bottom, and the
first match wins. The app learns rules from the user's own corrections instead of asking a model.

The assistant is an adapter behind `core/src/ai/`, disabled by default, that only sees movements no
rule matched. It proposes; the user accepts. Accepting is what turns a proposal into a rule.

## Consequences

- The app is fully functional with no model installed and no network.
- Categorisation is reproducible and explainable: every category can be traced to the rule that
  assigned it, and rules show their hit count.
- The model's answers are validated against the real category list, so a hallucinated category is
  dropped rather than stored.
- Users who want the assistant must install Ollama and pull a model; the UI reports whether the
  endpoint answers and warns when it is not loopback.
