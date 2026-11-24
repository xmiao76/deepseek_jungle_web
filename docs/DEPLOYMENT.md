# Deployment record

## Live site

- URL: <https://deepseek-jungle.pages.dev>
- Conformance host: <https://deepseek-jungle.pages.dev/conformance/>
- Cloudflare Pages project: `deepseek-jungle`
- Deployment method: `npx wrangler@4 pages deploy dist --project-name deepseek-jungle --branch main`
- Release: `deepseek-v4-1-flash-deepseek-harness-2.0.0.wasm` (SHA-256 `0e19711e8d2a06bfb174ce708b3c26c119054945fac1fc5984b29e650f1d952c`)
- Single file: `release/index.html` (see `release/SHA256.txt`)

## HTTP checks

| Check | Result |
|---|---|
| `GET /` | 200, HTML |
| `Content-Type` for the Arena wasm | `application/wasm` |
| `X-Content-Type-Options` | `nosniff` |
| `Referrer-Policy` | `strict-origin-when-cross-origin` |
| `/assets/*` cache | `public, max-age=31536000, immutable` |
| HTML cache | `public, max-age=0, must-revalidate` |
| Cross-origin isolation | not required (no threads/SharedArrayBuffer) |

## Browser verification (live, 2026-11-23)

Each browser booted the production URL (identity `DeepSeek v4.1 Flash · DeepSeek Harness`), started a
Human-vs-AI game, made a real move (Red elephant 42→35), and received the AI reply:

| Browser | Engine | Result |
|---|---|---|
| Chromium | worker | ply 2, AI reply 306 ms, no console errors |
| Microsoft Edge | worker | ply 2, AI reply 305 ms, no console errors |
| Firefox | worker | ply 2, AI reply 311 ms, no console errors |
| WebKit | worker | ply 2, AI reply 312 ms, no console errors |

The live **Download Arena .wasm** action produced
`deepseek-v4-1-flash-deepseek-harness-2.0.0.wasm` with the committed SHA-256. The live
`/conformance/` host completed a Red den win in 91 plies (max choose-move 48.7 ms, max render
15.5 ms, both boards every revision).

Screenshots: [`docs/screenshots/live-game.png`](screenshots/live-game.png),
[`docs/screenshots/live-conformance.png`](screenshots/live-conformance.png).

## Source publication

The refactor is committed locally on branch `Arena-WebAssembly` (the branch tracks
`origin/Arena-WebAssembly`; the previous release is tagged `web-v1.1.0`). Pushing with the
credentials available in this environment was rejected by GitHub (`403`, authenticated as
`xmiao08` rather than the repository owner `xmiao76`), so the commits above are local and must be
pushed by a repository owner. The deployed site and the committed release artifacts are
independent of that restriction.

## Post-deploy checklist

- [x] Public URL starts the new shell and loads its engine.
- [x] wasm served as `application/wasm`.
- [x] Download action byte-identical to the committed release.
- [x] Human vs AI full flow on the live site.
- [x] Conformance host on the live site completes a match.
- [x] Chrome, Edge, Firefox, WebKit (Playwright engines; Safari itself unavailable on Windows).
- [x] No COOP/COEP requirement; the game uses a classic worker.
