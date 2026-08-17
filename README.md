# Jungle (Dou Shou Qi / 斗兽棋) — Web Edition

A browser-based implementation of the classic Jungle board game with a visual board and a
built-in AI opponent. Play against the computer (or watch two AIs play) on a 7×9 board with
rivers, traps and dens.

**Live demo:** <https://deepseek-jungle.pages.dev>

Everything runs **client-side**: the C# game engine and AI from the Windows desktop app
(`deepseek_cc_jungle`) are compiled to WebAssembly via **Blazor WebAssembly (AOT)** and
deployed as a pure static site on **Cloudflare Pages** — no server, zero hosting fees.

## Architecture

- `src/JungleGame.Core` — **reused verbatim** from the desktop app (a pure .NET 8 class
  library: rules, engine, and AI in one project; its only package is
  `System.Collections.Immutable`). It is the single C# source of truth for the rules and
  the AI; nothing was reimplemented in JavaScript. `JungleGame.Bench` is ported too
  (portable, no file I/O) for native baselines and the CI self-play smoke.
- `src/JungleGame.Web` — Blazor WebAssembly standalone app (`net8.0`):
  - **Rendering:** Razor components with a CSS-grid board and inline SVG/CSS terrain
    (checkerboard land, gradient river with wave lines, ⚠ traps, 🏠 dens) — a recreation
    of the WPF code-behind design (`JungleGame.UI/Themes/Theme.xaml` +
    `Views/MainWindow.Rendering.cs` are the spec). Pieces are emoji + rank-pip discs with
    the animal's name, animated via CSS transitions.
  - **Game flow:** `Services/GameService.cs` is a synchronous core adapted from the desktop
    `MainViewModel` (modes, first move, selection, captured trays, status text); it tracks
    moves itself because the engine's `GameState.History` stores only Zobrist hashes.
    `Pages/Index.razor` owns the async AI turn loop.
  - **Threading:** `<WasmEnableThreads>true</WasmEnableThreads>` gives the app real pthreads
    in the browser. The AI search runs via `await Task.Run(() => ai.FindBestMove(state, token))`
    exactly like the desktop app, so the UI never freezes while the engine thinks. This
    requires the page to be **cross-origin isolated** (`SharedArrayBuffer`), which the host
    enables with `Cross-Origin-Opener-Policy: same-origin` /
    `Cross-Origin-Embedder-Policy: require-corp` headers — served by `wwwroot/_headers` on
    Cloudflare Pages and by `e2e/serve.mjs` locally. The header badge in the UI shows
    whether threading is active.
  - **Audio:** synthesized in the browser with Web Audio (`wwwroot/js/app.js`) — the
    desktop app has no sound assets, so the web edition creates them fresh. Move/capture/win
    sounds with a mute toggle; the context is unlocked on the first user gesture.
  - **Undo** is a web-only addition (the desktop has none): it replays the tracked move
    list against the pure, deterministic `GameController.ApplyMove`.
- `tests/` — the desktop xUnit suite (`JungleGame.Tests`: 128 cases incl. perft
  differentials against `Helpers/NaiveMoveGenerator.cs` and Debug-calibrated perf gates)
  carried over, plus `JungleGame.Web.Tests` (31 cases: xUnit + bUnit) covering the
  board↔display coordinate mapping incl. flip mode, the game-flow service (incl. undo and
  the human-side/first-move logic), and component rendering.
- `e2e/` — Playwright suite against the **published AOT bundle**: a cross-origin-isolation
  check, a scripted human-vs-AI game (moves, AI replies, selection indicators, undo), and a
  WASM-runtime smoke test in which two AIs play a **full game to a terminal state** in the
  browser (the analog of the desktop `JungleGame.Bench --selfplay` crash smoke).
- `.github/workflows/ci.yml` — build + all .NET tests + coverage gate (Core ≥ 80%,
  currently ~98%) + Bench self-play smoke + publish (AOT) + Playwright E2E on every
  PR/push; deploys to Cloudflare Pages on push to `main` when the
  `CLOUDFLARE_API_TOKEN`/`CLOUDFLARE_ACCOUNT_ID` secrets are configured.

### Browser performance (measured, Chromium, AOT)

The search is time-bounded per move (iterative deepening), so the three desktop difficulty
presets adapt to the browser automatically. The shipped default is the **Rust engine**
(`rust-jungle/`), a byte-exact port of the C# engine compiled to WebAssembly and run in a
Web Worker — it is ~19× faster than the C# AOT engine in the browser (the C# engine remains
the oracle and the fallback, selectable with `?engine=csharp`):

| Budget | Depth | Nodes | NPS |
|---|---|---|---|
| Easy, 300 ms | 9 | ~160k | ~530k |
| Medium, 1000 ms | ~12 | ~570k | ~570k |
| Hard, 2000 ms | 13 | ~1.5M | ~530k |

The old C# AOT browser engine (still the fallback) reached depth 8 at Hard (~30k nps); the
Rust engine's ~19× speedup is a ~5-ply advantage at equal time, directly translating to
stronger play. Desktop native baseline for comparison: depth 11 at ~870k nps in 2 s.

## Rules

Standard Jungle / Dou Shou Qi (<https://en.wikipedia.org/wiki/Jungle_(board_game)>), with
the same house rule as the desktop engine: the **Lion** jumps the river horizontally
(across the 2-wide span) and vertically (across the 3-tall span); the **Tiger** jumps
**horizontally only**; a rat in a crossed water square blocks the jump. Win: den entry or
eliminating all enemy pieces; stalemate (no legal move) is a **loss**; the only draw is
**threefold repetition** (there is no 100-ply rule in this engine). The authoritative
implementation is `src/JungleGame.Core/Rules/*` + `Engine/GameController.cs`,
regression-tested by the carried-over suite.

## Gameplay & controls

- **Select** one of your pieces by clicking it (gold outline). Legal moves appear as green
  dots; capturable enemies get a red ring. Click a highlighted square to move.
- Your side renders at the **bottom edge** of the board (Blue in human-first games, Red in
  AI-first games), identical to the desktop app — each player's Rat starts on their right,
  their Elephant on their left.
- **New game** restarts with the current settings. **Undo** takes back the last move — in
  Human vs AI it also undoes the AI's reply. **Flip board** rotates the view 180°
  (visual only: it never changes the game state, the sides, or whose turn it is). **Mute**
  silences sounds.
- **Game mode:** Human vs AI or AI vs AI (watch mode). **First move:** human-first or
  AI-first (Blue-first or Red-first in watch mode) — the human plays whichever side moves
  first, exactly like the desktop StartDialog.
- **AI difficulty:** Easy (300 ms), Medium (1000 ms), Hard (2000 ms) per move — the three
  desktop presets.
- The side panel shows the AI search info (depth · nodes · ms · nps), captured pieces per
  side, and the full move history.

## Run locally

Prerequisites: .NET 8 SDK and the WASM AOT workload (one-time):

```bash
dotnet workload install wasm-tools
```

```bash
dotnet watch --project src/JungleGame.Web        # dev server (fast iteration)
dotnet test JungleGame.sln -c Release            # all .NET test suites
```

Note: the `dotnet watch` dev server cannot send COOP/COEP headers, so AI thinking pauses the
UI there. To run locally exactly like production (AOT + background-thread AI):

```bash
dotnet publish src/JungleGame.Web -c Release
node e2e/serve.mjs                           # serves the publish output on http://127.0.0.1:8123 with COOP/COEP
```

E2E (Playwright; installs Chromium on first use):

```bash
cd e2e && npm install && npx playwright install chromium
npx playwright test
```

## Deploy (Cloudflare Pages)

The Pages project name derives from the repo name with underscores hyphenated and the
`-web` suffix dropped: `deepseek_jungle_web` → **`deepseek-jungle`** →
`https://deepseek-jungle.pages.dev`.

```bash
dotnet publish src/JungleGame.Web -c Release
npx wrangler pages deploy src/JungleGame.Web/bin/Release/net8.0/publish/wwwroot --project-name deepseek-jungle
```

or rely on the GitHub Actions workflow (needs `CLOUDFLARE_API_TOKEN` and
`CLOUDFLARE_ACCOUNT_ID` repository secrets).

Post-deploy verification checklist: load the public URL, confirm the header badge reads
*threading: active*, `curl -I https://deepseek-jungle.pages.dev/` shows the COOP/COEP
headers, and a full game completes against the AI. If framework files fail integrity
checks, re-publish with `-p:CompressionEnabled=false` (Cloudflare then compresses on the
fly) and redeploy.

## Model & code-agent statement

- **Model used:** DeepSeek V4 Pro.
- **Code agent used:** Claude Code (by Anthropic).

The task specification is preserved in [`prompt_C#.md`](prompt_C#.md). The desktop original
lives at `C:\GitHub\deepseek_cc_jungle`.
