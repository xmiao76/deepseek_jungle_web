# Arena WIT and metadata

The contract is copied verbatim from the Jungle Arena v1 compatibility contract into
[`wit/jungle.wit`](../wit/jungle.wit). The release binary is built from it and tested against it.

## World

- Package/interface version: `jungle:arena@1.0.0`.
- World `competitor`: import `jungle:arena/display@1.0.0`, export
  `jungle:arena/game@1.0.0`.
- `info` is callable before `init`; `init` is called once per instance; `choose-move` is called only
  for the seat to move in a playing position; `render` may be called repeatedly for any revision in
  either orientation.

## Metadata returned by the release

| Field | Value |
|---|---|
| title | `DeepSeek V4.1 · DeepSeek Harness` |
| game-version | `2.0.0` |
| model | `DeepSeek V4.1` |
| code-agent | `DeepSeek Harness` |
| model-platform | `official` |
| github-url | `https://github.com/xmiao76/deepseek_jungle_web` |
| engine-version | `2.0.0` |
| runtime | `Rust 1.94.1 · wasm32-unknown-unknown` |
| rules-id | `desktop-jungle-v2` |
| rules-digest | `a457bbc009e975a9eafe5ff1f8fa40ca2548f2a689b121c8c4ed99dd0f3fdc57` |

The title and release filename are generated from `identity/identity.json`; the platform `official`
is stored in metadata but omitted from both the title and filename, as the contract requires.

## Build and inspection

```bash
npm run build:engine
wasm-tools component wit release/deepseek-v4-1-deepseek-harness-2.0.0.wasm
wasm-tools validate release/deepseek-v4-1-deepseek-harness-2.0.0.wasm
npm run admission
```

`wasm-tools component wit` must print exactly:

```
world root {
  import jungle:arena/display@1.0.0;
  export jungle:arena/game@1.0.0;
}
```

plus the interface type definitions. Any other import, export, missing function or changed type is
an admission failure.

## Rendering contract

- Exactly one `present(448, 576, rgba)` call per `render`.
- RGBA8, four bytes per pixel, no row padding, top-to-bottom, every alpha byte 255.
- Total payload `1,032,192` bytes.
- `progress = 1000` is always a complete settled frame; a direct seek to any revision is
  reconstructed from the snapshot alone.
- The terminal snapshot is rendered with a result card.
- `flipped` changes only the view and pointer mapping; coordinates and move order stay unflipped.

## Submission

The single file to upload is `release/deepseek-v4-1-deepseek-harness-2.0.0.wasm`. The Arena computes its
own SHA-256; the committed hash is in `release/SHA256.txt` and `README.md`. Upload at
<https://jungle-leaderboard.pages.dev/arena/upload/>, wait for every stage, then choose a seat.
