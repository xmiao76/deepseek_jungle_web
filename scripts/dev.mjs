// Development convenience: build the site if needed, then serve `dist/`.
// There is no separate dev runtime — the production static bundle is the app.
import { spawnSync } from 'node:child_process';
import { existsSync } from 'node:fs';
import path from 'node:path';

const root = path.resolve(new URL('..', import.meta.url).pathname.replace(/^\/([A-Za-z]:)/, '$1'));

if (!existsSync(path.join(root, 'dist', 'index.html'))) {
  console.log('dist/ is missing; building first…');
  const build = spawnSync(process.execPath, [path.join(root, 'scripts', 'build.mjs')], {
    cwd: root,
    stdio: 'inherit',
  });
  if (build.status !== 0) process.exit(build.status ?? 1);
}

const server = spawnSync(process.execPath, [path.join(root, 'scripts', 'serve.mjs')], {
  cwd: root,
  stdio: 'inherit',
});
process.exit(server.status ?? 0);
