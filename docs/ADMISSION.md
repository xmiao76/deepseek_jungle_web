# Arena admission checklist

Release: `deepseek-v4-pro-claude-code-2.0.0.wasm`
SHA-256: `357bb306160aa714b6b5ee48cbd57ba5c141efb5f277bd60386f91fdb5b22a1c`

| # | Check | Result | Detail |
|---|---|---|---|
| 1. size <= 24 MiB | PASS | 2871712 bytes |
| 2. component preamble | PASS | 00 61 73 6d 0d 00 01 00 |
| 3. SHA-256 identity | PASS | 357bb306160aa714b6b5ee48cbd57ba5c141efb5f277bd60386f91fdb5b22a1c |
| 4a. wasm-tools component wit | PASS | ok |
| 4b. exact import/export contract | PASS | display in / game out |
| 5. jco-transpile 0.24.0 < 30 s | PASS | 1102 ms |
| 6. metadata | PASS | DeepSeek V4 Pro · Claude Code · 2.0.0 |
| 7. opening smoke + frames | PASS | index 9, 448×576 RGBA ×2 |

Run with `npm run admission`.
