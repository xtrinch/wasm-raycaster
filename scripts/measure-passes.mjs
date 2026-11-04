// Measures the HUD's per-pass render timings in a headless Chromium browser (Edge or Chrome).
// Start the dev server first (pnpm run dev), then run:
//   pnpm run measure [url] [screenshot dir]
// It reads the timings at the spawn view and after each of four quarter turns left, and
// saves a screenshot of each view so you can see what was on screen. Set BROWSER to the
// browser executable if it is not found.
import { spawn } from "node:child_process";
import { existsSync, mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

const url = process.argv[2] ?? "http://localhost:5173/wasm-raycaster";
const screenshotDir = process.argv[3] ?? join(tmpdir(), "raycaster-measure");

if (typeof WebSocket === "undefined") {
  throw new Error("This Node has no WebSocket; use Node 22, or Node 20 with --experimental-websocket");
}

const browserPath =
  process.env.BROWSER ??
  [
    "C:/Program Files (x86)/Microsoft/Edge/Application/msedge.exe",
    "C:/Program Files/Google/Chrome/Application/chrome.exe",
    "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
    "/usr/bin/google-chrome",
    "/usr/bin/chromium",
  ].find((path) => existsSync(path));
if (!browserPath) {
  throw new Error("No Edge or Chrome found; set BROWSER to the browser executable");
}

const port = 9333;
const profileDir = mkdtempSync(join(tmpdir(), "raycaster-profile-"));
const browser = spawn(browserPath, [
  "--headless=new",
  `--remote-debugging-port=${port}`,
  `--user-data-dir=${profileDir}`,
  "--window-size=1280,720",
  "--no-first-run",
  "about:blank",
]);

const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

// Connect to the browser's page over the DevTools protocol.
let page;
for (let attempt = 0; attempt < 50 && !page; attempt++) {
  try {
    const targets = await (await fetch(`http://127.0.0.1:${port}/json/list`)).json();
    page = targets.find((target) => target.type === "page");
  } catch {
    // The browser is still starting.
  }
  if (!page) await sleep(200);
}
if (!page) throw new Error("The browser did not open a page");

const ws = new WebSocket(page.webSocketDebuggerUrl);
await new Promise((resolve) => (ws.onopen = resolve));
let nextId = 1;
const pending = new Map();
ws.onmessage = (event) => {
  const message = JSON.parse(event.data);
  pending.get(message.id)?.(message.result);
  pending.delete(message.id);
};
const send = (method, params = {}) =>
  new Promise((resolve) => {
    const id = nextId++;
    pending.set(id, resolve);
    ws.send(JSON.stringify({ id, method, params }));
  });
const evaluate = async (expression) =>
  (await send("Runtime.evaluate", { expression, returnByValue: true })).result.value;
const leftArrow = (type) =>
  send("Input.dispatchKeyEvent", { type, windowsVirtualKeyCode: 37, key: "ArrowLeft", code: "ArrowLeft" });
// The HUD lines: FPS, min FPS, then one "<pass> <ms> ms" line per timed pass.
const hud = () =>
  evaluate(`[...document.querySelectorAll(".text-white")].map((line) => line.textContent).join(" | ")`);
const screenshot = async (name) => {
  const shot = await send("Page.captureScreenshot", { format: "png" });
  writeFileSync(join(screenshotDir, `${name}.png`), Buffer.from(shot.data, "base64"));
};

mkdirSync(screenshotDir, { recursive: true });
await send("Page.navigate", { url });
await sleep(12000); // load WASM and textures, then let the 60-update average fill
console.log(
  "environment:",
  await evaluate(
    "JSON.stringify({ crossOriginIsolated, threads: navigator.hardwareConcurrency, width: innerWidth, height: innerHeight })"
  )
);
console.log("spawn view:", await hud());
await screenshot("view0");
for (let turn = 1; turn <= 4; turn++) {
  await leftArrow("rawKeyDown");
  await sleep(625); // turning runs at 0.8π rad/s, so this is about a quarter turn
  await leftArrow("keyUp");
  await sleep(3000); // let the 60-update average settle
  console.log(`after ${turn} quarter turn(s) left:`, await hud());
  await screenshot(`view${turn}`);
}
console.log("screenshots:", screenshotDir);

ws.close();
browser.kill();
await sleep(500);
rmSync(profileDir, { recursive: true, force: true });
