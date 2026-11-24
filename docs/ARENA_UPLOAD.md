# Arena upload attempt

**Date:** 2026-11-23 (America/Los_Angeles)  
**Release:** `release/deepseek-v4-pro-claude-code-2.0.0.wasm`  
**SHA-256:** `357bb306160aa714b6b5ee48cbd57ba5c141efb5f277bd60386f91fdb5b22a1c`

## Attempt

The documented upload URL was opened in Chromium:

<https://jungle-leaderboard.pages.dev/arena/upload/>

Observation (confirmed with DOM inspection):

- HTTP 200, but the response body is the same 39,155-byte shell as `/` and `/arena/`.
- The rendered page is the **published benchmark leaderboard**, not an upload form.
- `input[type="file"]` count: **0**; the page body contains no `upload` control or admission-stage UI.
- There is no Upload link in the leaderboard navigation either.

Consequently no file could be selected and no admission stage could be run. The public instance
currently exposes the completed August 2026 benchmark leaderboard (which lists
`DeepSeek-V4-Pro-0813` second, with "Play" linking to this deployed site) rather than the per-entrant
Upload flow described in the brief.

Evidence: [`docs/screenshots/arena-upload-unavailable.png`](screenshots/arena-upload-unavailable.png).

## What was verified instead

The repository ships the independent acceptance path required by the brief:

- `npm run admission` passes all seven admission stages against the release file.
- `npm run test:conformance` runs the independent JS referee against the release component as Red,
  as Blue and against itself. All three complete on the board with both boards rendering every
  revision.
- The browser conformance host at <https://deepseek-jungle.pages.dev/conformance/> loads the exact
  release binary and the fixture and completes a match; the recorded run was a Red den win in 91
  plies, max choose-move 55.3 ms, max render 19.8 ms.
- The single-file `release/index.html` downloads bytes whose SHA-256 equals the committed release
  file in Chromium and all tested browsers.

Per the brief, if the Arena is unreachable, milestones 1–5 remain the acceptance path.
