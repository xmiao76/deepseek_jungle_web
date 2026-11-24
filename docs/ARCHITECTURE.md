# Architecture

## One core, two outputs

```
                    ┌─────────────────────────────────────────┐
                    │              jungle-core                │
                    │  board · movgen · v2 adjudication · eval │
                    │  PVS search · node/time limits · draws   │
                    └───────────────┬─────────────┬───────────┘
                                    │             │
                    ┌───────────────▼───┐     ┌───▼──────────────────┐
                    │   jungle-render   │     │  jungle-webengine    │
                    │ 448×576 vector UI │     │ raw C ABI, JS clock  │
                    │ fonts + animal art│     │ rules/search/render  │
                    └─────────┬─────────┘     └───────────┬──────────┘
                              │                           │
             ┌────────────────▼──────────────┐   ┌────────▼─────────────────┐
             │      jungle-component         │   │  web/ + Web Worker       │
             │ WIT entrant, display only     │   │  TypeScript shell        │
             └───────────────────────────────┘   └──────────────────────────┘
```

- **`jungle-core`** is the only rules implementation used by the game and by the Arena entrant.
  It owns the canonical opening, legal-list sorting, adjudication, repetition/100-ply counters,
  evaluation and PVS search.
- **`jungle-render`** draws the board, pieces, labels and result into an RGBA pixmap at any integer
  scale. It has no dependency on the shell; the component and the standalone wasm call it with the
  same `View` data.
- **`jungle-component`** is the Arena adapter and the only WIT-facing crate. It maps the official
  snapshot into `game::Snapshot`, runs a fresh deterministic search, and presents exactly one frame
  per `render` call.
- **`jungle-webengine`** is the standalone engine: a tiny C ABI over the same core and renderer,
  used by the Web Worker (or by the main thread when a worker is unavailable). Its only wasm import
  is `env.now_ms`; the clock is used only for the standalone time budget.
- **`jungle-fixture`** is a second WIT component used only by the conformance host. It always
  returns legal index 0 and draws a deliberately different checkerboard.
- **`jungle-cli`** is native-only: rules/perft inspection, PNG rendering, the fixture match, the
  frozen-baseline arena and diagnostic evaluation/search commands.

## Standalone runtime

1. The shell fetches the wasm (or decodes the embedded base64 in `release/index.html`), compiles it,
   and transfers the bytes to a classic Web Worker created from a same-origin script (hosted) or a
   Blob (single file).
2. The main thread holds the authoritative ply list. The worker owns a game instance and the
   renderer; commands are `new`, `load`, `apply`, `search`, `render`, `snapshot`.
3. The worker returns a compact binary snapshot (revision, board, history, legal list, status,
   counters) and an `ImageBitmap`/RGBA frame. The shell draws the frame to a canvas and renders the
   menu, history, captured trays and result card in DOM/CSS.
4. A search runs inside the worker, so the page stays responsive. Undo or restart while thinking
   terminates and recreates the worker with the truncated ply list; stale replies are rejected by
   a generation token.
5. If the worker cannot be created or crashes, the same wasm is instantiated on the main thread.
   Gameplay remains complete; the header shows `main-thread engine`.

The standalone wasm exports a fixed binary snapshot layout rather than JSON, keeping per-frame and
per-move traffic small. The shell's `web/src/protocol.ts` is the single parser.

## Arena component

- The component is a `wasm32-unknown-unknown` cdylib built with `wit-bindgen` and wrapped by
  `wasm-tools component new`. The final binary imports only `jungle:arena/display@1.0.0`.
- `info()` returns the generated identity constants; `init()` stores seat/difficulty/seed;
  `choose-move()` reconstructs the official state from the snapshot (replaying the supplied history
  when it is complete), scales the host node budget by difficulty (Easy 25%, Medium 50%, Hard 100%),
  searches with **fresh TT/repetition/ordering state**, and returns a checked index.
- `render()` rebuilds the settled position from the snapshot, draws at 448×576, and calls
  `display.present` exactly once. Rendering state is separate from strategy state, so extra frames,
  flips and direct seeks cannot change a later choice.
- There is no clock, filesystem, network, thread, WASI or OS randomness in the component. Bounded
  native iteration collects the game result and all counters.

## Determinism and draw semantics

- Search is node-counted: every visited position counts, including the root and repeat visits.
  Aborts happen at node boundaries; incomplete iterations are discarded and the best move from the
  deepest completed iteration is returned (with a legal fallback).
- The component clears the transposition table, ordering heuristics and repetition path per
  `choose-move`, so the result depends only on the snapshot, legal list, budget, difficulty and
  seed, not on previous calls or renders.
- Repetition counts positions (all 63 squares + side to move), including the start and the terminal
  position. The no-capture counter increments on quiet moves and resets on captures. Both are
  modelled in search and in the game layer.
- The exact rules profile is committed at `rules/desktop-jungle-v2.json`; `npm run check` hashes the
  bytes (including the final LF) and fails on any drift.

## Release pipeline

`npm run build` runs identity/theme generation, the rules digest gate, the Rust wasm builds, the
component wrapping/validation, esbuild bundles, the hosted site, the exact-bytes single-file build,
the conformance host, and the hash/README updates. `release/index.html` embeds the standalone wasm,
the component bytes, the worker source and all fonts; it makes no network requests when opened from
`file://`.

## Verification layers

| Layer | Location | Checks |
|---|---|---|
| Rust unit/integration | `engine/**` | portable cases, adjudication rows, opening handshake, perft, search node bounds/determinism, renderer frames, identity rules |
| JS referee unit | `tests/unit` | the same fixtures independently implemented |
| Rules parity | `tests/parity` | random games where JS and wasm agree on every board, legal list, counter and result |
| Admission | `scripts/admission.mjs` | file, header, contract, jco 0.24, metadata, smoke frames |
| Conformance | `scripts/conformance.mjs` | three complete matches (Red/Blue/self), both boards every revision, determinism, flip hashes |
| Browser host | `conformance/` | the exact release binary vs the fixture, driven by the independent referee |
| Playwright | `tests/e2e` | human/AI and AI/AI games, undo during thought, flip, mute, mobile, Hard p100, downloads, `file://` in Chromium/Firefox/WebKit |
