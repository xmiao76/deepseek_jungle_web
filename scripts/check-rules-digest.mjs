// Build gate for the canonical rules profile: exact bytes, final LF, no BOM,
// SHA-256 equal to the digest in jungle_prompt.md.
import { readFileSync } from 'node:fs';
import { createHash } from 'node:crypto';
import path from 'node:path';

const root = path.resolve(new URL('..', import.meta.url).pathname.replace(/^\/([A-Za-z]:)/, '$1'));
const file = path.join(root, 'rules/desktop-jungle-v2.json');
const bytes = readFileSync(file);
const expected = 'a457bbc009e975a9eafe5ff1f8fa40ca2548f2a689b121c8c4ed99dd0f3fdc57';

const problems = [];
if (bytes.length >= 3 && bytes[0] === 0xef && bytes[1] === 0xbb && bytes[2] === 0xbf) problems.push('BOM present');
if (bytes.length === 0 || bytes[bytes.length - 1] !== 0x0a) problems.push('missing final LF');
if (bytes.includes(0x0d)) problems.push('CRLF line endings present');
const digest = createHash('sha256').update(bytes).digest('hex');
if (digest !== expected) problems.push(`digest ${digest} != ${expected}`);

if (problems.length) {
  console.error(`rules digest gate FAILED: ${problems.join('; ')}`);
  process.exit(1);
}
console.log(`rules digest OK: ${digest}`);
