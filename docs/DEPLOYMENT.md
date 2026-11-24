# Deployment record

## Live site

- URL: <https://deepseek-jungle.pages.dev>
- Conformance host: <https://deepseek-jungle.pages.dev/conformance/>
- Cloudflare Pages project: `deepseek-jungle`
- Deployment method: `npx wrangler@4 pages deploy dist --project-name deepseek-jungle --branch main`
- Release: `deepseek-v4-pro-claude-code-2.0.0.wasm` (SHA-256 `357bb306160aa714b6b5ee48cbd57ba5c141efb5f277bd60386f91fdb5b22a1c`)
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

Each browser booted the production URL, started a Human-vs-AI game, made a real move (Red elephant
42→35), and received the AI reply:

| Browser | Engine | Result |
|---|---|---|
| Chromium | worker | ply 2, AI reply ~304 ms, no console errors |
| Microsoft Edge | worker | ply 2, AI reply ~304 ms, no console errors |
| Firefox | worker | ply 2, AI reply ~306 ms, no console errors |
| WebKit | worker | ply 2, AI reply ~312 ms, no console errors |

The live **Download Arena .wasm** action produced a file with the committed SHA-256. The live
`/conformance/` host completed a Red den win in 91 plies (max choose-move 55.3 ms, max render
19.8 ms, both boards every revision).

Screenshots: [`docs/screenshots/live-game.png`](screenshots/live-game.png),
[`docs/screenshots/live-conformance.png`](screenshots/live-conformance.png).

## Post-deploy checklist

- [x] Public URL starts the new shell and loads its engine.
- [x] wasm served as `application/wasm`.
- [x] Download action byte-identical to the committed release.
- [x] Human vs AI full flow on the live site.
- [x] Conformance host on the live site completes a match.
- [x] Chrome, Edge, Firefox, WebKit (Playwright engines; Safari itself unavailable on Windows).
- [x] No COOP/COEP requirement; the game uses a classic worker.
