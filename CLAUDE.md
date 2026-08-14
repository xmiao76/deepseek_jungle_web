# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project state

This repository is **pre-implementation**: it contains only the project specification, `prompt_C#.md` (the authoritative spec — read it before planning any work; it supersedes this summary wherever they differ). No code, README, or CI exists yet.

The project: a browser-based Jungle (Dou Shou Qi) game — human vs built-in AI, entirely client-side — built as a .NET 8 Blazor WebAssembly standalone app with AOT compilation and WASM threading, deployed as a static site on Cloudflare Pages (free tier).

The desktop source to port is **`C:\GitHub\deepseek_cc_jungle`** (.NET 8 + WPF). A sibling repo **`C:\GitHub\qwen_jungle_web`** is a completed, working web port of a *different* desktop implementation (`qwen_cc_jungle`) built to the same spec prompt — use it as the proven template for the web layer (Blazor project settings, `GameService`, `Index.razor` AI loop, `_headers`, `e2e/serve.mjs`, Playwright, CI), but adapt everything to the deepseek engine's API, which differs in important ways.

## Architecture (per the spec)

- **Port, don't rewrite, the engine.** `JungleGame.Core` (rules + engine + AI in ONE project — AI lives in its `AI/` subfolder, not a separate project) is copied **verbatim** from the desktop repo, along with its xUnit test project (`JungleGame.Tests`) and `JungleGame.Bench` (portable, no file I/O). The authoritative rule interpretation is the desktop code (`Rules/MoveValidator.cs`, `Rules/CaptureResolver.cs`, `Engine/GameController.cs` — win/draw logic lives in its private `CheckWinCondition`, there is no `WinChecker` class). No rule logic may be reimplemented in JavaScript.
- **Engine API reality** (differs from the qwen engine the original prompt assumed): no `AiConfig` class — difficulty is `SetTimeLimit(TimeSpan)`; `Move? FindBestMove(GameState, CancellationToken)` is sync/blocking and internally serialized via `lock(_searchGate)`; only `NodesSearched` and `LastCompletedDepth` are exposed — **no LastSearch struct, no score, no elapsed, no RandomSeed (fully deterministic)**. Sole package: `System.Collections.Immutable` 10.0.7. `InternalsVisibleTo("JungleGame.Tests")` — keep assembly names unchanged. TT is 1M entries ≈ 50MB allocated at runtime.
- Only the presentation layer is rewritten: WPF Canvas code-behind → Blazor Razor components (CSS grid + inline SVG), and the desktop `MainViewModel` game-flow logic → a plain C# service (drop `INotifyPropertyChanged`/WPF plumbing). See `JungleGame.UI/ViewModels/MainViewModel.cs` `DoAIMove` in the desktop repo for the background-AI pattern: `await Task.Run(() => _ai.FindBestMove(_state, token), token)` + CTS staleness guard (`if (token.IsCancellationRequested) return;`).
- **WASM threading is mandatory**: `<WasmEnableThreads>true</WasmEnableThreads>` (+ `RunAOT=true`, `EmccMaximumHeapSize=536870912`) so AI search runs on a real background thread without freezing the UI. Requires cross-origin isolation via COOP/COEP headers (see Deployment). The spec mandates a threading spike as the very first deliverable.
- No bitmap/audio assets exist to reuse — the desktop renders everything procedurally. Recreate the visual design in CSS/SVG using `JungleGame.UI/Themes/Theme.xaml` (palette) + `Views/MainWindow.Rendering.cs` (composition) as the spec, and synthesize fresh sound effects.

## Key behavioral constraints

- **User-confirmed decisions**: 3 difficulty presets (Easy 300 / Medium 1000 / Hard 2000 ms — desktop `StartDialog` values), NO Expert, NO thinking-time selector; 2 modes — Human vs AI (human-first or AI-first) and AI vs AI watch, NO Human-vs-Human; engine copied 100% verbatim (no added properties) — the search-info line shows depth/nodes/time/nps with time measured by the web layer.
- **The human plays whichever side moves first** (human-first → Blue at rows 0–2; AI-first → Red at rows 6–8). The desktop CLAUDE.md's "human always Blue" line is wrong — the code is authoritative.
- **Win/draw rules**: den invasion, elimination, stalemate = loss for the side to move, three-fold repetition = draw. **There is NO 100-ply rule** — do not document or implement one. Tiger jumps column-change only (across the 2-wide span); Lion both; rat blocks. (The desktop repo's own `prompt.md` tiger wording is garbled — `MoveValidator.ValidateJump` is authoritative.)
- **Undo** is a web-only addition (desktop has none): replay-based — rebuild from `GameState.CreateInitial()` by reapplying the tracked move list minus the last (last two in human modes). The web service must track moves externally (`GameState.History` holds only Zobrist hashes).
- Display mapping matches the desktop **screen** geometry (its `CellOrigin` paints visual row 0 at the canvas bottom), not its `GetVisualPosition` internal: unflipped display = `(c, 8−r)` so Blue renders at the bottom edge (rat g3 on the player's right, elephant a3 on the left); flipped display = `(6−c, r)`. Flip only remaps squares↔pixels; never mutate game state or sides.
- Desktop UI version is 1.1.0; the web project continues that numbering. Menu/README attribution: model **DeepSeek V4 Pro**, code agent **Claude Code**.
- No `perft-oracle.txt` and no `bench-baseline.md` exist — the oracle is `JungleGame.Tests/Helpers/NaiveMoveGenerator.cs` + `PerftTests` differentials; the native baseline (~870k nps, depth 11 in 2s) is recorded in the desktop CLAUDE.md.
- Desktop has no `--smoke-test` flag — the WASM smoke test mirrors `JungleGame.Bench --selfplay` (AI-vs-AI full game in the browser).

## Commands (target state)

One-time setup:
- `dotnet workload install wasm-tools` (required for AOT publish)

Build & test (engine + AI + web suites run on stock .NET 8, never under WASM):
- `dotnet build JungleGame.sln -c Release`
- `dotnet test JungleGame.sln -c Release` (perf gates `[Trait("Category","Perf")]` run by default; escape hatch if a runner is slow: `--filter "Category!=Perf"`)
- Coverage gate: `dotnet test tests/JungleGame.Tests --collect:"XPlat Code Coverage"` then `powershell -File scripts/check-coverage.ps1` (Core ≥ 80%)
- Native baseline / crash smoke: `dotnet run --project src/JungleGame.Bench -c Release -- --bench --time 2000` and `-- --selfplay --games 2 --timeA 1000 --timeB 300`

Local dev:
- `dotnet watch --project src/JungleGame.Web` — CAVEAT: the dev server cannot send COOP/COEP headers, so threading is unavailable and AI thinking pauses the UI there
- Production-like local run: `dotnet publish src/JungleGame.Web -c Release` then `node e2e/serve.mjs` (serves the publish output on http://127.0.0.1:8123 with COOP/COEP + br/gz negotiation)

E2E: `cd e2e && npm install && npx playwright install chromium` then `npx playwright test`

Deploy (static site, no server):
- `npx wrangler pages deploy src/JungleGame.Web/bin/Release/net8.0/publish/wwwroot --project-name deepseek-jungle`
- Project-name derivation (document in README): repo name shortened/hyphenated, `-web` dropped → `deepseek_jungle_web` → `deepseek-jungle` → `https://deepseek-jungle.pages.dev`
- If Pages serves pre-compressed `.br`/`.gz` framework files with wrong encoding: publish with `-p:CompressionEnabled=false`
- Post-deploy checklist: `self.crossOriginIsolated === true`, `curl -I` COOP/COEP + wasm MIME, at least one full game incl. AI moves and one AI-vs-AI game.

## Deployment / runtime constraints

- `wwwroot/_headers` (proven by the sibling port): COOP same-origin + COEP require-corp on `/*`; `Cache-Control: no-cache, no-store` on `/index.html` and `/_framework/blazor.boot.json`; `/_framework/*` = `no-cache, must-revalidate` (**NOT immutable** — .NET 8 framework files aren't content-hashed); `Content-Type: application/wasm` for `/_framework/*.wasm`.
- Site must work from any static-host path (no absolute paths; correct `<base href>`). First load is a multi-MB AOT download — `Blazor.start` hooks + loading overlay.
- `Directory.Build.props` (`TreatWarningsAsErrors`, `AnalysisLevel=latest-recommended`) and `.editorconfig` are carried verbatim from the desktop repo — new web code must comply; never relax them pre-emptively (scoped relaxation of the web project only as a last resort).

## Testing layers required by the spec

1. xUnit: the entire copied `JungleGame.Tests` (~128 cases: Engine 12, Rules 45, Model 9, AI 48 + 10 perft theory, Perf 2, Integration 4) on stock .NET 8 in CI.
2. bUnit (`tests/JungleGame.Web.Tests`): BoardCoordinates mapping incl. flip, GameService logic (modes, first-move → human side incl. human=Red when AI-first, undo, captured trays, move-history formatting, search-info formatting), BoardView rendering (63 squares, ⚠/🏠 markers, highlights).
3. Playwright WASM smoke: cross-origin isolation check + `?mode=aiai&diff=easy&pace=30` AI-vs-AI full game to a terminal `data-result`.
4. Playwright E2E: scripted human-vs-AI game (Blue cat `(1,1)→(1,2)`, dog `(5,1)→(5,2)`) against the published AOT bundle via `e2e/serve.mjs`.
5. GitHub Actions (ubuntu): build → test → coverage gate → Bench selfplay smoke → publish AOT → artifact → e2e job → deploy to Pages on push to main (`CLOUDFLARE_API_TOKEN`/`CLOUDFLARE_ACCOUNT_ID`).
