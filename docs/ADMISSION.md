# Arena admission checklist

Release: `deepseek-v4-1-flash-deepseek-harness-2.0.0.wasm`
SHA-256: `0e19711e8d2a06bfb174ce708b3c26c119054945fac1fc5984b29e650f1d952c`

| # | Check | Result | Detail |
|---|---|---|---|
| 1. size <= 24 MiB | PASS | 2871737 bytes |
| 2. component preamble | PASS | 00 61 73 6d 0d 00 01 00 |
| 3. SHA-256 identity | PASS | 0e19711e8d2a06bfb174ce708b3c26c119054945fac1fc5984b29e650f1d952c |
| 4a. wasm-tools component wit | PASS | ok |
| 4b. exact import/export contract | PASS | display in / game out |
| 5. jco-transpile 0.24.0 < 30 s | PASS | 1159 ms |
| 6. metadata | PASS | DeepSeek v4.1 Flash · DeepSeek Harness · 2.0.0 |
| 7. opening smoke + frames | PASS | index 9, 448×576 RGBA ×2 |

Run with `npm run admission`.
