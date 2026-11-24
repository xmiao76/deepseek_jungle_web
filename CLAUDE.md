# CLAUDE.md

This file is the working guide for agents touching this repository. The authoritative product
specification is [`jungle_prompt.md`](jungle_prompt.md); the previous .NET/Blazor implementation is
preserved in git history and tagged **`web-v1.1.0`**.

## Project state

The repository now delivers both outputs required by the brief from one Rust core:

1. A static browser game (`dist/`, Cloudflare Pages).
2. A self-contained WebAssembly **Component Model** Arena entrant
   (`release/deepseek-v4-1-flash-deepseek-harness-2.0.0.wasm`), plus the committed offline single-file
   `release/index.html`.

There is no .NET, Blazor, C# or tablebase in the shipped build. The frozen v1 engine core in
`bench/v1/jungle-baseline` is native-only and exists solely for the strength arena. The old
tablebase asset was dropped because it was generated under the buggy v1 trap-capture rule.

## Architecture

- `engine/crates/jungle-core` — board, movegen, v2 adjudication, evaluation, PVS search. The single
  rules/AI source of truth.
- `engine/crates/jungle-render` — 448×576 vector board/pieces/result renderer; shared by the
  standalone wasm and the Arena component.
- `engine/crates/jungle-component` — WIT entrant. Imports only `jungle:arena/display@1.0.0`.
- `engine/crates/jungle-webengine` — raw C ABI for the standalone worker; only wasm import is
  `env.now_ms`.
- `engine/crates/jungle-fixture` — conformance-host opponent, always returns legal index 0.
- `engine/crates/jungle-identity` — generated identity constants (title/filename/metadata).
- `engine/crates/jungle-cli` — native tools: rules/perft, PNG render, fixture match, baseline arena.
- `web/` — TypeScript shell, worker, HTML template, CSS. Boards are Rust-rendered; menus/history/
  result card are DOM.
- `conformance/` — browser host using the independent JS referee and the exact release binary.
- `tests/referee.mjs` — independent v2 rules implementation. `tests/fixtures/` are shared by Rust,
  the JS referee and the component tests.
- `scripts/` — identity/theme codegen, build, admission, conformance, strength, screenshots, serve.
- `release/` — committed artifacts, `SHA256.txt`, `RELEASE.json`.
- `docs/` — plan, architecture, WIT, conformance, strength, admission, screenshots, art credits.

## Hard invariants

- **Rules profile:** `rules/desktop-jungle-v2.json` must remain byte-identical (final LF, no BOM).
  `npm run check` fails otherwise. Canonical opening and Red-first; terminal order den → elimination →
  immobilization → repetition → no-capture; 100 quiet plies draw; trap-effective ranks before the
  rat/elephant exceptions; captures cross the water boundary in neither direction.
- **Identity:** edit only `identity/identity.json`; run `npm run sync`. Title/filename/metadata/menu/
  README are generated. Platform `official` is stored but omitted from title/filename.
- **Component constraints:** exactly one import (`display`) and one export (`game`); no WASI, clock,
  threads, files, network, OS randomness. A fresh TT/repetition/ordering state per `choose-move`
  keeps the choice deterministic across calls, renders and instances. Frames are exactly 448×576
  RGBA8, alpha 255, one `present` per `render`.
- **Difficulty budgets:** Arena node budget scaling Easy 25%, Medium 50%, Hard 100%. Standalone
  Easy ≈ 90 ms/depth 4, Medium ≈ 300 ms/depth 10, Hard ≈ 950 ms wall clock. Hard must reach the UI
  under 1 s including worker messaging; the Playwright suite measures p100.
- **Human/AI first:** the v2 first side is Red. `Human first` = human Red; `AI first` = AI Red, human
  Blue. The board auto-flips for a Blue human; flip is view-only.
- **Shared rendering:** do not create a second DOM/SVG board. The Arena frame and standalone canvas
  must use `jungle-render`; the standalone passes `show_result: false` and draws its DOM result card.
- **Release artifacts are committed.** Rebuild with `npm run build`; hashes and README marker blocks
  are updated by the build. Never hand-edit generated files (`identity.generated.ts`,
  `theme.generated.css`, `theme_tokens.rs`, `release/RELEASE.json`, `release/SHA256.txt`).

## Commands

```bash
npm ci
npm run build                 # sync, digest gate, Rust wasm, site, single file, hashes, host
npm run check                 # generated drift, rules digest, tsc
cargo test --workspace        # from engine/
npm test                       # node:test rules + JS/wasm parity
npm run admission              # 7 admission stages against the committed wasm
npm run test:conformance       # admission + three complete matches + frames/docs
npm run test:e2e               # Playwright (server auto-starts)
npm run strength               # baseline arena -> docs/STRENGTH.md
npm run serve                  # http://127.0.0.1:8123
```

`npm run build:engine` and `npm run build:web` are available separately after generated files and
Rust artifacts exist.

## Known pitfalls (learned during the refactor)

- The JS `String.replace` replacement **must be a function** when inlining app JS; `$&`/`` $` `` in
  the bundle otherwise injects the `<!--SCRIPT-->` marker and truncates the single-file script.
- Base64 decoding must account for `=` padding, or the downloaded component gains trailing bytes and
  the SHA-256 check fails.
- `jco transpile -I sync` uses the unversioned import key `"jungle:arena/display"`; the generated
  `.d.ts` says versioned. Use the key at runtime.
- Modal markup lives outside `#app`; click delegation is on `document`, and e2e closes modals with
  `.modal-close`, not the backdrop selector.
- The static server must map `/dir/` to `dir/index.html` for the conformance host.
- Root-move equal-score tie-breaking changes the engine's move choice and cost ~40% strength in
  testing. Keep the deterministic root ordering; do not reintroduce a lexicographic tie-break.
- The `Search` constructor now requires a TT size and separate `Limits`; the component uses
  `Limits::nodes`, the standalone passes a `now_ms` function.
- The old component tablebase is not used; do not re-add a v1 WDL table without regenerating it
  under v2 rules.
- `wasm-tools component wit` output synthesizes a `root:component` wrapper world; check only the
  import/export interface names and types, not the wrapper world name.

## Verification of record

- `docs/CONFORMANCE.md` — latest admission + three-match report and frame hashes.
- `docs/STRENGTH.md` — head-to-head vs the frozen v1 core and fixture results.
- `docs/screenshots/` — reviewed standalone, mobile, result, Arena frames, conformance host.
- Playwright Hard p100 is printed by the e2e suite; it must stay ≤ 1000 ms.
