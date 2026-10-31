import { chromium, openScene, startVite } from "./harness.mjs";
const [dir] = process.argv.slice(2);
const stop = await startVite();
const browser = await chromium.launch();
const { host } = await openScene(browser, dir, "01-editor");
console.log(JSON.stringify(host));
await browser.close();
stop();
