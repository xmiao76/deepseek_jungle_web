// Regenerate the reviewed screenshots used by the README and docs. The e2e
// tests are the source of truth for every screen; this runs the screenshot
// cases only.
import { spawnSync } from 'node:child_process';
import path from 'node:path';

const root = path.resolve(new URL('..', import.meta.url).pathname.replace(/^\/([A-Za-z]:)/, '$1'));
const playwright = path.join(root, 'node_modules', '@playwright', 'test', 'cli.js');
const result = spawnSync(
  process.execPath,
  [
    playwright,
    'test',
    'tests/e2e/standalone.spec.ts',
    '--project=chromium',
    '-g',
    'menu, identity|human move|AI vs AI completes|mobile layout',
  ],
  { cwd: root, stdio: 'inherit' },
);
process.exit(result.status ?? 1);
