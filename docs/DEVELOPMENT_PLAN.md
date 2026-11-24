# Development plan

The release was built in the order below. Each checkpoint produced a working artifact and was
verified before the next step.

1. **Toolchain spike.** A minimal WIT component with `info`/`render`, wrapped with
   `wasm-tools component new`, transpiled with `jco-transpile` 0.24.0, and a tiny-skia + embedded-text
   frame verified in Node. This confirmed the only-`display` import and the pinned toolchain before
   removing the old stack.
2. **Repository pivot.** Tagged the previous release `web-v1.1.0`; removed the Blazor/.NET projects;
   moved the Rust core into a workspace (`jungle-core`, render, identity, component, webengine,
   fixture, cli); froze the previous engine core under `bench/v1` for the strength arena; added the
   identity/theme generators and the rules digest gate.
3. **Rules v2.** Canonical opening and Red-first; capture precedence fixed for trapped defenders and
   water boundaries; game-level repetition/100-ply adjudication; sorted legal lists; the portable
   23 cases, 9 adjudication rows and opening handshake encoded as shared fixtures; independent JS
   referee; Rust and JS fixtures green.
4. **Search.** Node-budget/time-limit split; deterministic component mode; draw-aware search; a
   search-stability regression (a lexicographic root tie-break) was found by the frozen-baseline
   arena and removed; parity games and node-budget benchmarks green.
5. **Rendering.** Shared palette/theme generation, OFL fonts, original vector animal art, board,
   overlays, animation and result card; 448×576 component frames and scaled standalone frames tested
   for exact size, alpha, determinism and flip.
6. **Arena component.** WIT adapter, identity metadata, snapshot conversion, difficulty budget
   scaling and renderer; admissions and conformance host; three complete matches (Red, Blue, self)
   with both boards rendering every revision.
7. **Standalone shell.** Web Worker engine with main-thread fallback, menu/setup, game screen,
   two-click input, keyboard cursor, undo (including during thought via worker restart), flip, mute,
   WebAudio cues, loading progress, result card, downloads and identity panel.
8. **Single-file release.** Build-time inlining of CSS, both fonts, the standalone wasm, the worker
   and the exact component bytes; base64 padding bug fixed after a hash mismatch; `file://` verified
   in Chromium, Firefox and WebKit; download hash equals the committed file.
9. **Release and acceptance.** Full Playwright suite (Hard p100 954.8 ms), conformance evidence,
   strength arena against the frozen previous engine, README/architecture/WIT/credits docs, hashes,
   Cloudflare Pages deployment and the Arena upload attempt.

## Acceptance status

- [x] Rules digest gate and all conformance fixtures.
- [x] Admission checklist 8/8 (the seven contract stages with the contract split into inspection and
  exact-signature assertions).
- [x] Complete games as Red, as Blue and against itself; every revision rendered by both boards.
- [x] Determinism after renders and across fresh instances; flip-only view; opaque 448×576 frames.
- [x] Standalone Human-vs-AI and AI-vs-AI full games; undo during thought; Hard p100 under 1 s.
- [x] `release/index.html` over `file://` in Chromium, Firefox and WebKit, with download hash.
- [x] Hosted Cloudflare Pages deployment and live checks in Chromium, Edge, Firefox and WebKit.
- [x] Arena upload attempted; the public instance serves the benchmark leaderboard with no upload
  form or file input, recorded in `docs/ARENA_UPLOAD.md`. Milestones 1–5 are the acceptance path.

## Interpreted requirements

- "Human first / AI first" means **who controls the first-moving side**. The v2 first side is Red, so
  Human-first gives the human Red and AI-first gives the human Blue with the AI on Red. The board
  auto-flips for a Blue human; flip remains a pure view change.
- The previous release was not an Arena component, so the strength baseline is the frozen v1 engine
  core in `bench/v1`, played on canonical v2 positions with illegal v1 trap captures normalized to
  the first legal move. Results and confidence intervals are in `docs/STRENGTH.md`.
- The Arena's node budget is a ceiling; the component spends it only on visited positions and has no
  clock. Standalone Hard spends ~0.95 s of wall clock and leaves headroom for worker messaging.
