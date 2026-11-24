// One-command release build: sync generated identity/theme, build Rust
// artifacts, then build the hosted site and the single-file release.
import { spawnSync } from 'node:child_process';
import path from 'node:path';

const root = path.resolve(new URL('..', import.meta.url).pathname.replace(/^\/([A-Za-z]:)/, '$1'));

function run(script) {
  console.log(`\n=== ${script} ===`);
  const result = spawnSync(process.execPath, [path.join(root, 'scripts', script)], {
    cwd: root,
    stdio: 'inherit',
  });
  if (result.status !== 0) process.exit(result.status ?? 1);
}

run('sync-identity.mjs');
run('gen-theme.mjs');
run('check-rules-digest.mjs');
run('build-engine.mjs');
run('build-web.mjs');
