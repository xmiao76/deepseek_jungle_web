// Builds the hosted static site, the committed single-file release and both
// hashes from the Rust artifacts. Run via `npm run build` (engine first).
import { spawnSync } from 'node:child_process';
import { copyFileSync, createReadStream, existsSync, mkdirSync, readFileSync, readdirSync, rmSync, statSync, writeFileSync } from 'node:fs';
import { createHash } from 'node:crypto';
import path from 'node:path';
import * as esbuild from 'esbuild';

const root = path.resolve(new URL('..', import.meta.url).pathname.replace(/^\/([A-Za-z]:)/, '$1'));
const buildDir = path.join(root, 'build');
const dist = path.join(root, 'dist');
const release = path.join(root, 'release');
const identity = JSON.parse(readFileSync(path.join(root, 'identity/identity.json'), 'utf8'));
const filename = JSON.parse(readFileSync(path.join(release, 'RELEASE.json'), 'utf8')).filename;

function fail(message) {
  console.error(message);
  process.exit(1);
}
if (!existsSync(path.join(buildDir, 'jungle-component.wasm'))) fail('run npm run build:engine first');

function sha256(bytes) {
  return createHash('sha256').update(bytes).digest('hex');
}
function b64(bytes) {
  return bytes.toString('base64');
}

rmSync(dist, { recursive: true, force: true });
mkdirSync(path.join(dist, 'assets', 'fonts'), { recursive: true });
mkdirSync(path.join(release, 'screenshots'), { recursive: true });

// Worker and app bundles.
const workerOut = path.join(buildDir, 'engine-worker.js');
await esbuild.build({
  entryPoints: [path.join(root, 'web/src/engine-worker.ts')],
  bundle: true,
  format: 'iife',
  platform: 'browser',
  target: ['es2020'],
  minify: true,
  legalComments: 'none',
  outfile: workerOut,
});
await esbuild.build({
  entryPoints: [path.join(root, 'web/src/main.ts')],
  bundle: true,
  format: 'iife',
  platform: 'browser',
  target: ['es2020'],
  minify: true,
  legalComments: 'none',
  outfile: path.join(dist, 'assets/app.js'),
});

const cssSource =
  readFileSync(path.join(root, 'web/src/theme.generated.css'), 'utf8') +
  '\n' +
  readFileSync(path.join(root, 'web/src/styles.css'), 'utf8');
const fonts = {
  __FONT_MARCELLUS__: 'Marcellus-Regular.ttf',
  __FONT_LATO_REGULAR__: 'Lato-Regular.ttf',
  __FONT_LATO_BOLD__: 'Lato-Bold.ttf',
  __FONT_LATO_BLACK__: 'Lato-Black.ttf',
};
for (const file of Object.values(fonts)) {
  copyFileSync(path.join(root, 'assets/fonts', file), path.join(dist, 'assets/fonts', file));
}
let hostedCss = cssSource;
for (const [placeholder, file] of Object.entries(fonts)) {
  hostedCss = hostedCss.replaceAll(placeholder, `./fonts/${file}`);
}
writeFileSync(path.join(dist, 'assets/app.css'), hostedCss);

copyFileSync(workerOut, path.join(dist, 'assets/engine-worker.js'));
copyFileSync(path.join(buildDir, 'jungle_webengine.wasm'), path.join(dist, 'assets/jungle_webengine.wasm'));
const componentBytes = readFileSync(path.join(buildDir, 'jungle-component.wasm'));
const componentHash = sha256(componentBytes);
copyFileSync(path.join(buildDir, 'jungle-component.wasm'), path.join(dist, 'assets', filename));
copyFileSync(path.join(buildDir, 'jungle-fixture.wasm'), path.join(dist, 'assets/fixture.wasm'));

const template = readFileSync(path.join(root, 'web/index.html'), 'utf8');
const hostedHtml = template
  .replace('<!--STYLE-->', () => '<link rel="stylesheet" href="./assets/app.css" />')
  .replace('<!--WORKER-->', () => '<script>window.__JUNGLE_WORKER_URL__ = "./assets/engine-worker.js";</script>')
  .replace('<!--SCRIPT-->', () => '<script src="./assets/app.js"></script>');
writeFileSync(path.join(dist, 'index.html'), hostedHtml);
writeFileSync(
  path.join(dist, '_headers'),
  `/*
  X-Content-Type-Options: nosniff
  Referrer-Policy: strict-origin-when-cross-origin
  Permissions-Policy: geolocation=(), microphone=(), camera=()

/index.html
  Cache-Control: no-cache, no-store, must-revalidate

/assets/*
  Cache-Control: public, max-age=31536000, immutable
`,
);

// Single-file release: every byte inline, no sibling or network references.
const workerSource = readFileSync(workerOut, 'utf8');
const appSource = readFileSync(path.join(dist, 'assets/app.js'), 'utf8');
let singleCss = cssSource;
for (const [placeholder, file] of Object.entries(fonts)) {
  const bytes = readFileSync(path.join(root, 'assets/fonts', file));
  singleCss = singleCss.replaceAll(placeholder, `data:font/ttf;base64,${b64(bytes)}`);
}
const wasmBytes = readFileSync(path.join(buildDir, 'jungle_webengine.wasm'));
const appHash = sha256(Buffer.from(appSource));
const embedded = JSON.stringify({
  wasmBase64: b64(wasmBytes),
  componentBase64: b64(componentBytes),
  sha256: componentHash,
  filename,
  appHash,
});
const escapeScript = (text) => text.replaceAll('</script', '<\\/script');
const singleHtml = template
  .replace('<!--STYLE-->', () => `<style>\n${singleCss}\n</style>`)
  .replace('<!--WORKER-->', () => `<script type="text/plain" id="worker-source">${escapeScript(workerSource)}</script>`)
  .replace(
    '<!--SCRIPT-->',
    () =>
      `<script>window.__JUNGLE_EMBEDDED__ = ${embedded};</script>\n<script>\n${escapeScript(appSource)}\n</script>`,
  );
const singleBytes = Buffer.from(singleHtml);
writeFileSync(path.join(release, 'index.html'), singleBytes);
copyFileSync(path.join(buildDir, 'jungle-component.wasm'), path.join(release, filename));
const singleHash = sha256(singleBytes);
writeFileSync(
  path.join(release, 'SHA256.txt'),
  `${singleHash}  release/index.html\n${componentHash}  release/${filename}\n`,
);

// RELEASE.json and README hash block.
const releaseJson = {
  ...JSON.parse(readFileSync(path.join(release, 'RELEASE.json'), 'utf8')),
  sha256: { index: singleHash, component: componentHash },
  bytes: { index: singleBytes.length, component: componentBytes.length },
  standaloneWasmSha256: sha256(wasmBytes),
};
writeFileSync(path.join(release, 'RELEASE.json'), JSON.stringify(releaseJson, null, 2) + '\n');

const readmePath = path.join(root, 'README.md');
if (existsSync(readmePath)) {
  const start = '<!-- release:start -->';
  const end = '<!-- release:end -->';
  const block = `${start}
| Artifact | SHA-256 | Bytes |
|---|---|---|
| \`release/index.html\` | \`${singleHash}\` | ${singleBytes.length.toLocaleString('en-US')} |
| \`release/${filename}\` | \`${componentHash}\` | ${componentBytes.length.toLocaleString('en-US')} |

The rules profile is \`${identity.rulesId}\` with digest \`${identity.rulesDigest}\`.
${end}`;
  const readme = readFileSync(readmePath, 'utf8');
  if (readme.includes(start) && readme.includes(end)) {
    writeFileSync(readmePath, readme.replace(new RegExp(`${start}[\\s\\S]*?${end}`), block));
  }
}

// Local static server helper metadata.
writeFileSync(
  path.join(dist, 'assets', 'release.json'),
  JSON.stringify({ filename, sha256: componentHash, size: componentBytes.length, rulesId: identity.rulesId }, null, 2),
);

// Browser conformance host: transpile the release and fixture components with
// the pinned jco-transpile, then copy the independent-referee host.
const jcoCli = path.join(root, 'node_modules', '@bytecodealliance', 'jco', 'dist', 'jco.js');
const conformance = path.join(dist, 'conformance');
mkdirSync(conformance, { recursive: true });
function transpile(component, outName) {
  const out = path.join(conformance, 'generated', outName);
  rmSync(out, { recursive: true, force: true });
  mkdirSync(out, { recursive: true });
  const result = spawnSync(
    process.execPath,
    [jcoCli, 'transpile', component, '-o', out, '-I', 'sync', '--name', outName, '--quiet'],
    { cwd: root, encoding: 'utf8' },
  );
  if (result.status !== 0) throw new Error(`jco transpile failed: ${result.stderr || result.stdout}`);
  return { dir: `./generated/${outName}`, name: outName, coreFiles: readdirSync(out).filter((f) => f.endsWith('.wasm')) };
}
const releaseEntry = transpile(path.join(release, filename), 'release');
const fixtureEntry = transpile(path.join(buildDir, 'jungle-fixture.wasm'), 'fixture');
const manifest = {
  release: {
    ...releaseEntry,
    label: 'Release',
    filename,
    sha256: componentHash,
    info: {
      title: releaseJson.title,
      model: releaseJson.model,
      codeAgent: releaseJson.codeAgent,
      modelPlatform: releaseJson.modelPlatform,
      rulesId: identity.rulesId,
      rulesDigest: identity.rulesDigest,
    },
  },
  fixture: { ...fixtureEntry, label: 'Fixture' },
};
writeFileSync(path.join(conformance, 'manifest.json'), JSON.stringify(manifest, null, 2));
copyFileSync(path.join(root, 'conformance', 'index.html'), path.join(conformance, 'index.html'));
copyFileSync(path.join(root, 'conformance', 'host.js'), path.join(conformance, 'host.js'));
copyFileSync(path.join(root, 'tests', 'referee.mjs'), path.join(conformance, 'referee.mjs'));

console.log(`dist: ${dist}`);
console.log(`release/index.html: ${(singleBytes.length / 1024 / 1024).toFixed(2)} MiB  sha256=${singleHash}`);
console.log(`release/${filename}: ${(componentBytes.length / 1024 / 1024).toFixed(2)} MiB  sha256=${componentHash}`);
console.log(`standalone wasm: ${(wasmBytes.length / 1024).toFixed(0)} KiB  sha256=${sha256(wasmBytes)}`);
