# Art, audio and third-party credits

## Original work

- Board composition, terrain (parchment, jade river, dens, traps), piece tokens, rank pips, labels,
  selection/legal/capture indicators, last-move trail, result card and the Arena frame are original
  vector artwork drawn procedurally by `engine/crates/jungle-render` (tiny-skia paths). Animal
  emblems (rat, cat, dog, wolf, leopard, tiger, lion, elephant) are original silhouettes.
- All sound effects (select, move, capture, win, lose, draw) are synthesized at runtime with the
  Web Audio API; there are no sampled or third-party sounds.
- The standalone UI, menus, "How to play", markup, CSS and animations are original work.

## Fonts

Both fonts are redistributed under the SIL Open Font License 1.1. The licence texts are included
next to the binaries in [`assets/fonts`](../assets/fonts).

| Font | Use | Licence |
|---|---|---|
| **Marcellus** Regular | display headings, result titles, wordmark | SIL OFL 1.1 (`Marcellus-OFL.txt`) |
| **Lato** Regular / Bold / Black | UI, move history, piece labels, rank pips | SIL OFL 1.1 (`Lato-OFL.txt`) |

Font sources: the Google Fonts project (`ofl/marcellus`, `ofl/lato`).

## Runtime and build tools

- Rust 1.94.1, `wasm32-unknown-unknown`
- `wasm-tools` 1.263.0
- `wit-bindgen` 0.63.0, `tiny-skia` 0.12.0, `ab_glyph` 0.2.32
- `@bytecodealliance/jco` 1.42.0 with `@bytecodealliance/jco-transpile` 0.24.0
- Node 22+, esbuild 0.28.2, TypeScript 5.9.3, Playwright 1.62.1

Licence texts for these tools are provided by their upstream distributions. The Jungle rules and
Arena WIT contract are defined by the task brief in [`jungle_prompt.md`](../jungle_prompt.md).
