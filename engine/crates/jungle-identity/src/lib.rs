//! GENERATED from identity/identity.json by scripts/sync-identity.mjs.
//! Do not edit by hand; run `npm run sync` instead.

pub const MODEL: &str = "DeepSeek V4 Pro";
pub const CODE_AGENT: &str = "Claude Code";
pub const MODEL_PLATFORM: &str = "official";
pub const GITHUB_URL: &str = "https://github.com/xmiao76/deepseek_jungle_web";
pub const GAME_VERSION: &str = "2.0.0";
pub const ENGINE_VERSION: &str = "2.0.0";
pub const RUNTIME: &str = "Rust 1.94.1 · wasm32-unknown-unknown";
pub const RULES_ID: &str = "desktop-jungle-v2";
pub const RULES_DIGEST: &str = "a457bbc009e975a9eafe5ff1f8fa40ca2548f2a689b121c8c4ed99dd0f3fdc57";
pub const RUST_TOOLCHAIN: &str = "1.94.1";
pub const TITLE: &str = "DeepSeek V4 Pro · Claude Code";
pub const RELEASE_FILENAME: &str = "deepseek-v4-pro-claude-code-2.0.0.wasm";

/// The platform segment is present only when the platform is non-empty and not
/// the literal `official`.
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

/// Lowercase; each run of characters outside `a-z0-9` becomes one hyphen;
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

pub fn release_filename_for(model: &str, code_agent: &str, platform: &str, version: &str) -> String {
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
        assert_eq!(title_for("Claude-Opus-5.5", "ClaudeCode", "official"), "Claude-Opus-5.5 · ClaudeCode");
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
        assert_eq!(release_filename_for("M", "A", "", "1.0.0"), "m-a-1.0.0.wasm");
    }

    #[test]
    fn generated_values_are_consistent() {
        assert_eq!(title(), TITLE);
        assert_eq!(release_filename(), RELEASE_FILENAME);
        assert_eq!(slug("  DeepSeek  V4_Pro!! "), "deepseek-v4-pro");
        assert_eq!(RELEASE_FILENAME, "deepseek-v4-pro-claude-code-2.0.0.wasm");
        assert_eq!(TITLE, "DeepSeek V4 Pro · Claude Code");
    }
}
