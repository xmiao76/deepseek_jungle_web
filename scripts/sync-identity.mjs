// Generates the one identity constants module used by the component metadata,
// the standalone menu, the release filename and the README. Run via
// `node scripts/sync-identity.mjs`; `--check` fails when generated files drift.
import { readFileSync, writeFileSync, existsSync } from 'node:fs';
import path from 'node:path';

const root = path.resolve(new URL('..', import.meta.url).pathname.replace(/^\/([A-Za-z]:)/, '$1'));
const check = process.argv.includes('--check');
const identity = JSON.parse(readFileSync(path.join(root, 'identity/identity.json'), 'utf8'));

const required = ['model', 'codeAgent', 'modelPlatform', 'githubUrl', 'gameVersion', 'engineVersion', 'runtime', 'rulesId', 'rulesDigest'];
for (const key of required) {
  if (typeof identity[key] !== 'string') throw new Error(`identity.${key} must be a string`);
}
if (!identity.githubUrl.startsWith('https://github.com/')) throw new Error('githubUrl must start https://github.com/');
if (identity.model.length > 160 || identity.codeAgent.length > 160 || identity.modelPlatform.length > 160) {
  throw new Error('identity metadata parts must be at most 160 characters');
}

const platformSegment = (platform) => {
  const p = platform.trim();
  return p && p !== 'official' ? p : null;
};

const title = [identity.model.trim(), identity.codeAgent.trim()];
if (platformSegment(identity.modelPlatform)) title.push(platformSegment(identity.modelPlatform));
const titleText = title.join(' · ');

const slug = (value) =>
  value
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-+|-+$/g, '');

const nameParts = [slug(identity.model), slug(identity.codeAgent)];
if (platformSegment(identity.modelPlatform)) nameParts.push(slug(platformSegment(identity.modelPlatform)));
const releaseFilename = `${nameParts.join('-')}-${identity.gameVersion}.wasm`;

const rustString = (value) => JSON.stringify(value);
const rust = `//! GENERATED from identity/identity.json by scripts/sync-identity.mjs.
//! Do not edit by hand; run \`npm run sync\` instead.

pub const MODEL: &str = ${rustString(identity.model)};
pub const CODE_AGENT: &str = ${rustString(identity.codeAgent)};
pub const MODEL_PLATFORM: &str = ${rustString(identity.modelPlatform)};
pub const GITHUB_URL: &str = ${rustString(identity.githubUrl)};
pub const GAME_VERSION: &str = ${rustString(identity.gameVersion)};
pub const ENGINE_VERSION: &str = ${rustString(identity.engineVersion)};
pub const RUNTIME: &str = ${rustString(identity.runtime)};
pub const RULES_ID: &str = ${rustString(identity.rulesId)};
pub const RULES_DIGEST: &str = ${rustString(identity.rulesDigest)};
pub const RUST_TOOLCHAIN: &str = ${rustString(identity.rustToolchain ?? '')};
pub const TITLE: &str = ${rustString(titleText)};
pub const RELEASE_FILENAME: &str = ${rustString(releaseFilename)};

/// The platform segment is present only when the platform is non-empty and not
/// the literal \`official\`.
pub fn platform_title_segment(platform: &str) -> Option<&str> {
    let p = platform.trim();
    if p.is_empty() || p == "official" {
        None
    } else {
        Some(p)
    }
}

pub fn title_for(model: &str, code_agent: &str, platform: &str) -> String {
    let mut title = format!("{} · {}", model.trim(), code_agent.trim());
    if let Some(segment) = platform_title_segment(platform) {
        title.push_str(" · ");
        title.push_str(segment);
    }
    title
}

pub fn title() -> String {
    title_for(MODEL, CODE_AGENT, MODEL_PLATFORM)
}

/// Lowercase; each run of characters outside \`a-z0-9\` becomes one hyphen;
/// hyphens are trimmed at both edges.
pub fn slug(segment: &str) -> String {
    let mut out = String::with_capacity(segment.len());
    let mut pending_hyphen = false;
    for ch in segment.chars() {
        let lower = ch.to_ascii_lowercase();
        if lower.is_ascii_lowercase() || lower.is_ascii_digit() {
            if pending_hyphen && !out.is_empty() {
                out.push('-');
            }
            pending_hyphen = false;
            out.push(lower);
        } else {
            pending_hyphen = true;
        }
    }
    out
}

pub fn release_filename_for(
    model: &str,
    code_agent: &str,
    platform: &str,
    version: &str,
) -> String {
    let mut name = format!("{}-{}", slug(model), slug(code_agent));
    if let Some(segment) = platform_title_segment(platform) {
        name.push('-');
        name.push_str(&slug(segment));
    }
    format!("{name}-{version}.wasm")
}

pub fn release_filename() -> String {
    release_filename_for(MODEL, CODE_AGENT, MODEL_PLATFORM, GAME_VERSION)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn brief_examples() {
        assert_eq!(
            title_for("Claude-Opus-5.5", "ClaudeCode", "official"),
            "Claude-Opus-5.5 · ClaudeCode"
        );
        assert_eq!(
            title_for("Claude-Opus-5.5", "ClaudeCode", "SomeProvider"),
            "Claude-Opus-5.5 · ClaudeCode · SomeProvider"
        );
        assert_eq!(
            release_filename_for("Claude-Opus-5.5", "ClaudeCode", "official", "1.1.0"),
            "claude-opus-5-5-claudecode-1.1.0.wasm"
        );
        assert_eq!(
            release_filename_for("Claude-Opus-5.5", "ClaudeCode", "SomeProvider", "1.1.0"),
            "claude-opus-5-5-claudecode-someprovider-1.1.0.wasm"
        );
    }

    #[test]
    fn empty_platform_is_omitted() {
        assert_eq!(title_for("M", "A", ""), "M · A");
        assert_eq!(
            release_filename_for("M", "A", "", "1.0.0"),
            "m-a-1.0.0.wasm"
        );
    }

    #[test]
    fn generated_values_are_consistent() {
        assert_eq!(title(), TITLE);
        assert_eq!(release_filename(), RELEASE_FILENAME);
        assert_eq!(slug("  DeepSeek  v4.1_Flash!! "), "deepseek-v4-1-flash");
        assert_eq!(
            RELEASE_FILENAME,
            ${rustString(releaseFilename)}
        );
        assert_eq!(TITLE, ${rustString(titleText)});
    }
}
`;

const ts = `// GENERATED from identity/identity.json by scripts/sync-identity.mjs.
// Do not edit by hand; run \`npm run sync\` instead.

export const IDENTITY = {
  model: ${JSON.stringify(identity.model)},
  codeAgent: ${JSON.stringify(identity.codeAgent)},
  modelPlatform: ${JSON.stringify(identity.modelPlatform)},
  githubUrl: ${JSON.stringify(identity.githubUrl)},
  gameVersion: ${JSON.stringify(identity.gameVersion)},
  engineVersion: ${JSON.stringify(identity.engineVersion)},
  runtime: ${JSON.stringify(identity.runtime)},
  rulesId: ${JSON.stringify(identity.rulesId)},
  rulesDigest: ${JSON.stringify(identity.rulesDigest)},
  title: ${JSON.stringify(titleText)},
  releaseFilename: ${JSON.stringify(releaseFilename)},
} as const;

export const ARENA_URL = 'https://jungle-leaderboard.pages.dev/arena/';
export const ARENA_UPLOAD_URL = 'https://jungle-leaderboard.pages.dev/arena/upload/';
`;

const releaseJson = {
  title: titleText,
  filename: releaseFilename,
  version: identity.gameVersion,
  model: identity.model,
  codeAgent: identity.codeAgent,
  modelPlatform: identity.modelPlatform,
  githubUrl: identity.githubUrl,
  engineVersion: identity.engineVersion,
  runtime: identity.runtime,
  rulesId: identity.rulesId,
  rulesDigest: identity.rulesDigest,
};

// Preserve build-computed hashes so `--check` is stable after a release build.
const releasePath = path.join(root, 'release/RELEASE.json');
if (existsSync(releasePath)) {
  try {
    const existing = JSON.parse(readFileSync(releasePath, 'utf8'));
    for (const key of ['sha256', 'bytes', 'standaloneWasmSha256']) {
      if (existing[key] !== undefined) releaseJson[key] = existing[key];
    }
  } catch {
    // a malformed file will be replaced by the generated record
  }
}

const outputs = [
  ['engine/crates/jungle-identity/src/lib.rs', rust],
  ['web/src/identity.generated.ts', ts],
  ['release/RELEASE.json', JSON.stringify(releaseJson, null, 2) + '\n'],
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

// README identity/release block.
const readmePath = path.join(root, 'README.md');
if (existsSync(readmePath)) {
  const start = '<!-- identity:start -->';
  const end = '<!-- identity:end -->';
  const block = `${start}
| Field | Value |
|---|---|
| Title | ${titleText} |
| Model | ${identity.model} |
| Code agent | ${identity.codeAgent} |
| Model platform | ${identity.modelPlatform} |
| Game version | ${identity.gameVersion} |
| Engine version | ${identity.engineVersion} |
| Runtime | ${identity.runtime} |
| Rules ID | ${identity.rulesId} |
| Rules digest | \`${identity.rulesDigest}\` |
| GitHub | <${identity.githubUrl}> |
| Release file | \`release/${releaseFilename}\` |
${end}`;
  const current = readFileSync(readmePath, 'utf8');
  if (current.includes(start) && current.includes(end)) {
    const next = current.replace(new RegExp(`${start}[\\s\\S]*?${end}`), block);
    if (check) {
      if (next !== current) {
        console.error('stale README identity block');
        failed = true;
      }
    } else if (next !== current) {
      writeFileSync(readmePath, next);
      console.log('updated README identity block');
    }
  }
}

if (check && failed) process.exit(1);
if (!check) console.log(`identity: ${titleText} -> ${releaseFilename}`);
