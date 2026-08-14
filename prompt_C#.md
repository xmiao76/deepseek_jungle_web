# Prompt: Jungle (Dou Shou Qi) — Web UI Edition (C#)

Act as a senior software architect and technical lead.

Create a step-by-step development plan for a **browser-based** Jungle board game application with a web UI and built-in AI, so a human can play against the computer on a visual board — running **entirely client-side** with the C# game engine and AI compiled to WebAssembly via **.NET Blazor WebAssembly (AOT)**, and deployed as a **static site on Cloudflare Pages (free tier)** so the game can be demoed on a public URL with zero hosting fees.

This is a web port of an existing, proven Windows desktop implementation located at `C:\GitHub\deepseek_cc_jungle` (.NET 8 + WPF). The desktop solution's `JungleGame.Core` project (rules + engine + AI in one class library — no separate AI project) is a pure .NET 8 class library — no WPF/Windows dependencies, one pure-managed NuGet package (`System.Collections.Immutable` 10.0.7), no file I/O — and MUST be reused as-is. Only the presentation layer (WPF/XAML views, view-model bindings, input, main loop) is rewritten for the web, as Blazor Razor components rendering with CSS/SVG.

Use the standard Jungle / Dou Shou Qi rules from this page as the main game specification and reference:
https://en.wikipedia.org/wiki/Jungle_(board_game)

If the board layout, terrain layout, or initial piece positions are unclear from the wiki page, also refer to:
https://veryspecial.us/free-downloads/AncientChess.com-DouShouQi.pdf

Do not restate the full rules in detail unless needed. The authoritative rule interpretation is the one already implemented and regression-tested in the desktop codebase (`src/JungleGame.Core/Rules/MoveValidator.cs`, `CaptureResolver.cs`, `Engine/MoveGenerator.cs`, plus the private win/draw logic in `Engine/GameController.cs` `CheckWinCondition`); the web version must behave identically.

## Architecture requirements

- **No server-side compute.** The deployed artifact is a static site: HTML, CSS, JavaScript, the .NET WebAssembly runtime and AOT-compiled assemblies, and audio assets. All game logic and AI search run in the visitor's browser.
- **Blazor WebAssembly standalone** app targeting `net8.0` (the same TFM as `JungleGame.Core`), with **Ahead-of-Time compilation** (`<RunAOT>true</RunAOT>`, which requires `dotnet workload install wasm-tools`). AOT is mandatory for Release builds: the interpreter-only runtime is far too slow for a strong search.
- **The AI search must run off the UI thread.** Enable .NET WebAssembly threading (`<WasmEnableThreads>true</WasmEnableThreads>`) and serve the site with `Cross-Origin-Opener-Policy: same-origin` and `Cross-Origin-Embedder-Policy: require-corp` headers (via a Cloudflare Pages `_headers` file) so `SharedArrayBuffer` is available. With real pthread support, the desktop pattern — `await Task.Run(() => ai.FindBestMove(state, token))` with a `CancellationTokenSource` and an engine-pinning staleness guard (see `src/JungleGame.UI/ViewModels/MainViewModel.cs`, `DoAIMove`) — works almost verbatim in the browser, and the UI thread never freezes while the AI thinks. The app has no cross-origin assets, so `require-corp` blocks nothing. Verify this early with the threading spike in the Requirements section.
- Recommended split (adjust with justification if a better structure exists):
  - UI layer: Blazor Razor components — board rendering (CSS grid + inline SVG/CSS), click/touch input, animations, audio (Web Audio / `<audio>`), menus, game-state display.
  - Game-flow service: a plain C# service adapted from the desktop `MainViewModel` (drop the WPF-only `Dispatcher`/`INotifyPropertyChanged` plumbing): owns the authoritative `GameState` + `MinimaxEngine`, drives AI turns on the background thread, applies moves, and raises the state changes the Razor components render.
- **Port, don't rewrite, the engine.** Copy `src/JungleGame.Core` into the web repository unchanged (audited: every API it uses — `Stopwatch`, seeded `Random(42)` for Zobrist, `Interlocked`, `lock`, `CancellationToken`, `ReadOnlySpan`, `ImmutableDictionary`/`ImmutableList` — works under WASM). Bring the xUnit test project and its helpers along as the regression oracle. Do not fork rule logic into JavaScript or reimplement any of it — a single C# source of truth prevents rule drift.
- **There are no bitmap or audio assets to reuse.** The desktop app renders everything procedurally in code-behind onto a Canvas (no image or sound files exist anywhere in the repo except the app icon). Recreate its visual design in CSS/SVG using `src/JungleGame.UI/Themes/Theme.xaml` (the palette) and `src/JungleGame.UI/Views/MainWindow.Rendering.cs` (the composition) as the specification, and create small sound effects fresh (see UI requirements).
- The desktop `JungleGame.Bench` CLI IS ported: unlike other desktop-only code it has no file I/O, and it provides (a) the native NPS/depth baseline for the README performance table and (b) the CI engine crash-smoke self-play. No opening book files are involved.

## Requirements

- Phase 1 must deliver a working web UI where a human can play against the AI in a modern desktop browser (Chrome, Edge, Firefox, Safari — current versions; .NET WASM threading requires cross-origin isolation, which all of them support).
- Before Phase 1 UI work, run a **threading spike**: publish a minimal Blazor WASM page with `WasmEnableThreads`, confirm `self.crossOriginIsolated === true` both locally and on Cloudflare Pages (headers applied), and confirm a ~2-second `MinimaxEngine.FindBestMove` on a background thread leaves a CSS animation running smoothly. If cross-origin isolation proves unachievable on the host, stop and propose a fallback (e.g. cooperative yielding inside the search) instead of silently shipping a frozen UI.
- The engine is fast natively but slower in the browser even with AOT, so AI difficulty budgets must be measured in the browser: the desktop baseline is ≈ 870k nps / depth 11 in 2 seconds (recorded in the desktop `CLAUDE.md`; there is no `bench-baseline.md` file). Keep the desktop's three difficulty presets (see below) — the engine's time-bounded iterative deepening degrades depth gracefully at browser NPS. The engine is fully deterministic (the only randomness is the fixed-seed `Random(42)` Zobrist table); no seed configuration exists or is needed.
- Three difficulty levels (Easy 300 ms / Medium 1000 ms / Hard 2000 ms per move — the desktop `StartDialog` presets) and two game modes: **Human vs AI** (with a who-moves-first choice: human-first or AI-first — the human plays whichever side moves first, Blue when human-first, Red when AI-first) and **AI vs AI** (watch mode). No Human-vs-Human, no Expert preset, no thinking-time selector — the desktop feature set is the spec.
- Show a clear loading screen with progress indication while the .NET runtime and assemblies download and boot (first load is several seconds and a multi-MB download); use `Blazor.start` hooks (`beforeStart`/`afterStarted`) with a CSS overlay. The game must be interactive immediately after boot completes.
- The site must work when served from any static host path (no absolute-path assumptions; set `<base href>` correctly). Being fully functional offline after first load is acceptable but not required.
- Automated tests must be created and maintained during development.
- Bugs found in testing or gameplay must be fixed and regression-tested until stable.
- The final application must complete full Jungle games correctly in the browser.

## Rule clarification for river jumping

- Implement the lion and tiger river-jump behavior exactly as in the desktop engine (its implemented and tested interpretation):
- The lion can jump across the river both column-changing (across the 2-wide span) and row-changing (across the 3-tall span) to the next non-water square on the opposite side.
- The tiger can jump column-changing only (across the 2-wide span) and must not jump row-changing (some traditional descriptions state the opposite; this project follows the desktop engine's implemented and tested interpretation — see `MoveValidator.cs` `ValidateJump`).
- A rat on any water square along the jump path blocks the jump.
- Rat/water boundary rules, trap rank-zeroing, and all capture rules must match `MoveValidator`/`CaptureResolver`/`GameController` exactly. Since the engine is reused, this is guaranteed by construction — do not reimplement these rules in JavaScript.

## UI requirements

- The final UI must be polished and attractive, not just functional — suitable for a public demo.
- Recreate the desktop visual design in CSS/SVG: checkerboard land tinted by zone, river with ripple accents, ⚠-glyph traps, 🏠-glyph dens (the desktop `Theme.xaml` palette and `MainWindow.Rendering.cs` composition are the spec — see the palette values recorded in the desktop's `Themes/Theme.xaml`: window `#252540→#1A1A28`, land `#E8C897`/`#DFC08D`, river `#4169E1→#2149B1`, traps `#C94F4F` with `#FFEB3B` marker, dens `#7A0000` with `#B30000` frame, Blue `#4169E1`/`#2149B1`/`#6E8EF0`, Red `#DC143C`/`#B01030`/`#FF4D6D`, selection gold `#FFD700`, legal-move green `#00CC00`, capture red `#FF4444`).
- Each piece should look like its animal: reuse the desktop representation — the animal emoji (🐁🐈🐕🐺🐆🐯🦁🐘) on a side-coloured disc with a rank-digit pip and the animal's short English name (e.g. "Rat", "Lion") in a small font on or under the piece, readable on both sides' colors at any board size and following the piece during move animations. (The desktop has no RAT/CAT/... letter codes — do not invent them.)
- The main menu must display which AI model and which code agent were used to build the game
  (e.g. "Made with model … · code agent Claude Code"), alongside the app/engine version —
  define a version constant in the web project (the desktop UI release is v1.1.0; the web
  project continues that numbering) — and runtime info (.NET WASM, AOT, threading status).
- The main menu must include a "How to play" section with simple but complete guidance for a
  brand-new player: the win conditions (den entry, elimination, stalemate is a loss) and the
  draw rules (threefold repetition — there is NO 100-ply rule in this engine), how to select and move
  pieces with the UI's input model and move/capture indicators, the full rank order and the
  Rat-vs-Elephant exception, river/swimming rules, Lion and Tiger river jumps (including
  rat-blocking), trap behavior, the own-den restriction, and the in-game controls (undo,
  board flip, mute).
- Include good usability details: piece selection highlights, legal move indicators, capture feedback, move animations, turn display, move history panel, captured-piece trays, an AI search-info line (depth / nodes / time / nps — from `MinimaxEngine.LastCompletedDepth` and `NodesSearched` plus a web-layer `Stopwatch`; the engine exposes no score and must not be modified to add one), and win/loss messaging.
- Support flipping the board upside down as a view option. This must only change the visual mapping of squares to pixels and mouse/touch coordinates back to squares (`(c, r) → (6-c, 8-r)`, the desktop `GetVisualPosition` mapping). It must not change game state, must not swap sides internally, and must not change whose turn it is.
- Support a clear UI option to choose whether the human player or the AI moves first; it must work correctly together with the board-flip option. The human plays the side of the first move (Blue when human-first, Red when AI-first) — do not force a fixed human side.
- Support an **Undo** control (a web-only addition: the desktop app has none). Undo must take back the last move — in Human vs AI it also undoes the AI's reply, returning the turn to the human. Implement it by replaying the tracked move list against the immutable `GameController.ApplyMove` (the engine's Zobrist hashing is deterministic, so replay reproduces the exact state); the web game-flow service must therefore track moves itself, because `GameState.History` stores only Zobrist hashes.
- Sound effects for move / capture / win with a mute toggle. No sounds exist in the desktop app — create small fresh assets (synthesized Web Audio tones or small audio files). Browsers require a user gesture before audio — handle this gracefully.
- Responsive layout: excellent on desktop; usable on tablet/mobile (touch input for the two-click select-then-move model) is a stretch goal for Phase 2, not Phase 1.
- Avoid placeholder-style visuals in the final release.

## Testing requirements

- The reused C# engine tests (xUnit; a single `JungleGame.Tests` project with ~128 cases: Engine 12, Rules 45, Model 9, AI 48 + 10 perft theory cases, 2 perf gates, Integration 4) must be brought over and kept passing in CI on stock .NET 8 — they validate the single source of truth. The regression oracle for rules/AI is the in-repo `Helpers/NaiveMoveGenerator.cs` plus the `PerftTests` differential counts (there is NO `perft-oracle.txt` file — do not invent one). Preserve the `[CollectionDefinition("Perf", DisableParallelization = true)]` on the time-budgeted perf gates; they are Debug-calibrated and pass with wide margin under CI's Release build.
- Create a GitHub Actions CI pipeline (the source repo's CI is Windows-only; the web repo's runs on ubuntu): build + `dotnet test` on every PR and push to main, coverage gate (the ported `scripts/check-coverage.ps1`, Core ≥ 80%), the `JungleGame.Bench --selfplay` crash smoke, plus deployment to Cloudflare Pages on push to main (via `CLOUDFLARE_API_TOKEN`/`CLOUDFLARE_ACCOUNT_ID` secrets).
- Add a WASM-runtime smoke test that exercises the engine under the actual browser WebAssembly runtime — e.g. a Playwright-driven AI-vs-AI self-play in the app (mirroring the desktop's `JungleGame.Bench --selfplay` crash smoke; the desktop UI has no `--smoke-test` mode) asserting a full game reaches a valid terminal state without exceptions. This catches WASM-specific breakage of the reused engine.
- Add bUnit (or equivalent) tests for the UI layer: board square↔pixel coordinate mapping including flipped mode, and the game-flow service's mode/first-move/undo logic (including human=Red when AI moves first).
- Add at least one end-to-end browser test (Playwright) that loads the deployed bundle, waits for the .NET runtime to boot, plays a scripted sequence of human moves against the AI, and asserts the game reaches a valid terminal state.

## Deployment requirements

- Deploy to **Cloudflare Pages free tier** as a static site (no Functions, no Workers paid features, no server).
- Derive the Pages project name (and therefore the `*.pages.dev` URL) from the meaningful part
  of the repository/project name, shortened and hyphenated: DNS subdomains cannot contain
  underscores, and the `-web` suffix is dropped since Pages already implies a web deployment,
  so this repo (`deepseek_jungle_web`) deploys as project `deepseek-jungle`
  at `https://deepseek-jungle.pages.dev`. Document this naming derivation in the README.
- Provide the exact local build + deploy workflow:
  - one-time setup: `dotnet workload install wasm-tools`
  - local dev: `dotnet run` / `dotnet watch` on the Blazor project — NOTE: the dev server cannot send COOP/COEP headers, so threading is unavailable and the AI pauses the UI there; for production-like local runs publish and serve with the repo's `e2e/serve.mjs` static server (applies the same headers as Pages)
  - production build: `dotnet publish src/JungleGame.Web/JungleGame.Web.csproj -c Release` (AOT runs here); the deployable folder is `wwwroot` inside the publish output
  - deploy: `npx wrangler pages deploy <publish-output>/wwwroot --project-name deepseek-jungle`, or the Cloudflare Pages git integration.
- Include a `wwwroot/_headers` file that (a) sets `Cross-Origin-Opener-Policy: same-origin` and `Cross-Origin-Embedder-Policy: require-corp` on all responses (required for WASM threading), (b) sets `Content-Type: application/wasm` for `.wasm` files, and (c) sets `Cache-Control: no-cache, no-store` on `index.html` and `blazor.boot.json`, and `no-cache, must-revalidate` on `_framework/*` (.NET 8 framework files are NOT content-hashed — `blazor.boot.json` carries integrity hashes instead — so they must revalidate on every deploy; do NOT give them long-lived immutable caching).
- Known pitfall: Blazor publish emits pre-compressed `.br`/`.gz` framework files that some static hosts serve with the wrong encoding, causing integrity/decoding failures in the browser. If this appears on Pages, publish with `-p:CompressionEnabled=false` and let Cloudflare compress on the fly; verify with `curl -I` on `_framework` files.
- Verify the deployed site after each deployment, not only the local build: load the public URL in a browser, confirm `self.crossOriginIsolated === true`, check headers/MIME types with `curl -I` (COOP/COEP present, wasm content type correct), and complete at least one full game including AI moves (plus one AI-vs-AI watch game).
- The repository must include a `README.md` with: live demo URL, how to run locally, how to deploy, gameplay/controls notes, architecture overview (Blazor WASM + AOT + background-thread AI design), browser-vs-native performance table, and a clear statement identifying which model and which code agent were used to complete the task (model: DeepSeek V4 Pro; code agent: Claude Code).
- Save this prompt as `prompt_C#.md` in the codebase.

## Please provide

- recommended tech stack details and justification (Blazor WASM rendering choice: CSS grid + inline SVG vs Canvas; any needed JS interop; how component state is managed)
- architecture and module breakdown (Razor UI components vs game-flow service vs reused engine; how the background-thread AI turn is wired and cancelled)
- engine reuse plan (what is copied verbatim, what is adapted — e.g. the `MainViewModel` game-flow logic minus `INotifyPropertyChanged`/WPF plumbing, the web-only undo and external move tracking — and where the app/version constant is defined)
- phased roadmap (including the threading spike as the first deliverable)
- test plan for each phase
- automated testing strategy (xUnit under stock .NET 8 in CI, WASM smoke test, bUnit UI tests, Playwright E2E)
- AI/engine performance plan for the browser (AOT, measured NPS vs the ~870k nps desktop baseline, the three desktop difficulty budgets, worker/thread lifecycle, first-load experience)
- local build, run, and test workflow
- bug-fix and regression-test workflow
- Cloudflare Pages deployment plan (headers, compression pitfall, CI deploy) and post-deploy validation plan
- expected repository contents
- suggested README.md contents
- completion criteria

Completion is only achieved when the game is playable and stable in the browser, completes full games correctly, passes required automated tests, is deployed and verified on a public Cloudflare Pages URL, includes README.md with the model/code-agent statement, and includes prompt_C#.md in the codebase.
