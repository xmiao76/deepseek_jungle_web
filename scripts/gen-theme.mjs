// Generates the Rust palette constants and the web CSS custom properties from
// design/theme.json, so the component and the shell share one visual identity.
import { readFileSync, writeFileSync, existsSync } from 'node:fs';
import path from 'node:path';

const root = path.resolve(new URL('..', import.meta.url).pathname.replace(/^\/([A-Za-z]:)/, '$1'));
const check = process.argv.includes('--check');
const theme = JSON.parse(readFileSync(path.join(root, 'design/theme.json'), 'utf8'));

const rustName = (key) =>
  key
    .replace(/([a-z0-9])([A-Z])/g, '$1_$2')
    .replace(/-/g, '_')
    .toUpperCase();

const rust = `//! GENERATED from design/theme.json by scripts/gen-theme.mjs.
//! Do not edit by hand.

${Object.entries(theme.colors)
  .map(([key, hex]) => `pub const ${rustName(key)}: u32 = 0x${hex.replace('#', '').toLowerCase()};`)
  .join('\n')}
`;

const css = `/* GENERATED from design/theme.json by scripts/gen-theme.mjs. */
:root {
${Object.entries(theme.colors)
  .map(([key, hex]) => `    --${key.replace(/[A-Z]/g, (m) => '-' + m.toLowerCase())}: ${hex};`)
  .join('\n')}
    --font-display: "${theme.fonts.display}", "Iowan Old Style", "Palatino Linotype", serif;
    --font-ui: "${theme.fonts.ui}", "Segoe UI", system-ui, sans-serif;
}
`;

const outputs = [
  ['engine/crates/jungle-render/src/theme_tokens.rs', rust],
  ['web/src/theme.generated.css', css],
];

let failed = false;
for (const [rel, content] of outputs) {
  const file = path.join(root, rel);
  if (check) {
    const current = existsSync(file) ? readFileSync(file, 'utf8') : '';
    if (current !== content) {
      console.error(`stale generated file: ${rel}`);
      failed = true;
    }
  } else {
    writeFileSync(file, content);
    console.log(`wrote ${rel}`);
  }
}
if (check && failed) process.exit(1);
