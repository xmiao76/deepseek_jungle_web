//! Fixture opponent for the conformance host: always returns legal index 0 and
//! draws a deliberately distinct board.

wit_bindgen::generate!({
    path: "../../../wit",
    world: "competitor",
});

use exports::jungle::arena::game::{Guest, Metadata, PlyMove, Settings, Snapshot};
use jungle::arena::display;

struct Fixture;

impl Guest for Fixture {
    fn info() -> Metadata {
        Metadata {
            title: "Jungle Conformance Fixture · Harness".to_string(),
            game_version: "1.0.0".to_string(),
            model: "Fixture".to_string(),
            code_agent: "Conformance Harness".to_string(),
            model_platform: "official".to_string(),
            github_url: "https://github.com/xmiao76/deepseek_jungle_web".to_string(),
            engine_version: "1.0.0".to_string(),
            runtime: "Rust 1.94.1 · wasm32-unknown-unknown".to_string(),
            rules_id: "desktop-jungle-v2".to_string(),
            rules_digest: "a457bbc009e975a9eafe5ff1f8fa40ca2548f2a689b121c8c4ed99dd0f3fdc57"
                .to_string(),
        }
    }

    fn init(_config: Settings) {}

    fn choose_move(_state: Snapshot, _legal: Vec<PlyMove>, _budget: u32) -> u32 {
        0
    }

    fn render(_state: Snapshot, flipped: bool, _progress: u16) {
        const W: u16 = 448;
        const H: u16 = 576;
        let mut rgba = vec![0u8; W as usize * H as usize * 4];
        for y in 0..H as usize {
            for x in 0..W as usize {
                let cell = ((y / 64) + (x / 64)) % 2 == 0;
                let idx = (y * W as usize + x) * 4;
                let (r, g, b) = if cell {
                    (232, 212, 232)
                } else {
                    (120, 48, 110)
                };
                rgba[idx] = if flipped { b } else { r };
                rgba[idx + 1] = g;
                rgba[idx + 2] = if flipped { r } else { b };
                rgba[idx + 3] = 255;
            }
        }
        display::present(W, H, &rgba);
    }
}

export!(Fixture);
