// Chromium speed gate for the spike: loads the wasm bench page and prints the
// measured nps. Gate: >= 40k nps (the spike's go/no-go line).
import { chromium } from "playwright";

const browser = await chromium.launch();
const page = await browser.newPage();
await page.goto("http://127.0.0.1:8124/");
await page.waitForFunction(() => document.body.dataset.result, null, { timeout: 30000 });
const out = await page.textContent("#out");
const result = await page.getAttribute("body", "data-result");
console.log(out);
console.log(`GATE: ${result}`);
const nps = Number((result?.match(/nps=(\d+)/) ?? [])[1] ?? 0);
await browser.close();
process.exit(nps >= 40000 ? 0 : 1);
