# 0001 — A local-first desktop app built with Tauri

**Status:** accepted

## Context

The product handles bank movements: the most sensitive personal data most people have. The
requirement is absolute — nothing leaves the user's machine — while the interface needs a dashboard
of draggable, resizable widgets with charts and dense tables.

Three shapes were considered:

- **Native macOS (SwiftUI).** Best-looking on the target machine and access to on-device models, but
  macOS-only, which cripples a public repository, and a widget grid has to be built from scratch.
- **Browser-only web app.** Zero install and instantly demoable, but persistence lives at the mercy
  of the browser's site data, PDF handling is limited and any model access requires a third-party
  API key.
- **Tauri 2 desktop app.** A real binary on macOS, Windows and Linux, a Rust core for the financial
  logic, and the web platform for the part it is genuinely best at: a grid of interactive widgets.

## Decision

Tauri 2, with a Rust core crate that has no UI dependency and a React + TypeScript frontend.

## Consequences

- The project mixes two stacks, so the repository combines the `web` and `generic` conventions.
- Contributors need both a Rust toolchain and Node, plus the platform's Tauri prerequisites.
- The binary stays around 13 MB instead of the ~100 MB an Electron equivalent would cost.
- Because the core is UI-free, the engine can be tested — and reused — without a webview.
