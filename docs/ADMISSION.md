# Arena admission checklist

Release: `deepseek-v4-1-deepseek-harness-2.0.0.wasm`
SHA-256: `3f7e3ffcba0c0452d9b5508c4023470ccc108472f93339010e66ec4269edb467`

| # | Check | Result | Detail |
|---|---|---|---|
| 1. size <= 24 MiB | PASS | 2871712 bytes |
| 2. component preamble | PASS | 00 61 73 6d 0d 00 01 00 |
| 3. SHA-256 identity | PASS | 3f7e3ffcba0c0452d9b5508c4023470ccc108472f93339010e66ec4269edb467 |
| 4a. wasm-tools component wit | PASS | ok |
| 4b. exact import/export contract | PASS | display in / game out |
| 5. jco-transpile 0.24.0 < 30 s | PASS | 1115 ms |
| 6. metadata | PASS | DeepSeek V4.1 · DeepSeek Harness · 2.0.0 |
| 7. opening smoke + frames | PASS | index 9, 448×576 RGBA ×2 |

Run with `npm run admission`.
