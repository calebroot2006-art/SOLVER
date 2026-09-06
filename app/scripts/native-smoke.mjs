import assert from "node:assert/strict";
import { execFile, spawn } from "node:child_process";
import { createHash } from "node:crypto";
import { access, mkdir, readFile, writeFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { promisify } from "node:util";
import { setTimeout as delay } from "node:timers/promises";
import {
  assertCommandDenied,
  assertExternalRequestBlocked,
  assertPageLoaded,
  assertStandardToken,
  externalProbeUrl,
} from "./runtime-evidence.mjs";

const appDir = fileURLToPath(new URL("..", import.meta.url));
const outputDir = path.join(appDir, "test-results");
const binary = path.join(appDir, "src-tauri/target/release/app.exe");
const nativeDriver = process.env.TAURI_TEST_EDGE_DRIVER;
const webviewFolder = process.env.TAURI_TEST_WEBVIEW_FOLDER;
const driverUrl = "http://127.0.0.1:4444";
const runFile = promisify(execFile);
await mkdir(outputDir, { recursive: true });

const evidence = {
  commit: process.env.GITHUB_SHA ?? "local",
  timestamp: new Date().toISOString(),
  driver: "Microsoft Edge WebDriver (direct WebView2 session)",
  stage: "setup",
  passed: false,
};
let driver;
let sessionId;
let driverLog = "";

async function processSnapshot() {
  if (!driver?.pid) return [];
  const result = await runFile(
    "pwsh.exe",
    [
      "-NoProfile",
      "-File",
      path.join(appDir, "scripts/driver-processes.ps1"),
      "-DriverProcessId",
      String(driver.pid),
    ],
    { windowsHide: true, timeout: 10_000, maxBuffer: 1_000_000 },
  );
  return JSON.parse(result.stdout);
}

async function request(method, endpoint, body, timeoutMs = 30_000) {
  const response = await fetch(`${driverUrl}${endpoint}`, {
    method,
    headers: { "Content-Type": "application/json" },
    body: body === undefined ? undefined : JSON.stringify(body),
    signal: AbortSignal.timeout(timeoutMs),
  });
  const data = await response.json();
  if (!response.ok || data.value?.error) {
    throw new Error(`WebDriver ${endpoint}: ${JSON.stringify(data.value)}`);
  }
  return data.value;
}

function command(method, endpoint, body) {
  assert.ok(sessionId, "A WebDriver session is required");
  return request(method, `/session/${sessionId}${endpoint}`, body);
}

async function waitUntil(check, timeoutMs, label) {
  const deadline = Date.now() + timeoutMs;
  let lastError;
  while (Date.now() < deadline) {
    try {
      const result = await check();
      if (result) return result;
    } catch (error) {
      lastError = error;
    }
    await delay(200);
  }
  throw new Error(`Timed out waiting for ${label}`, { cause: lastError });
}

try {
  assert.equal(process.platform, "win32", "This check targets the Windows app");
  const token = await runFile("whoami.exe", ["/groups", "/fo", "csv", "/nh"], {
    windowsHide: true,
    timeout: 10_000,
  });
  evidence.tokenGroups = token.stdout.trim().split(/\r?\n/);
  assertStandardToken(token.stdout);
  assert.ok(nativeDriver && webviewFolder, "Run setup-webdriver.ps1 first");
  await access(binary);
  evidence.binarySha256 = createHash("sha256")
    .update(await readFile(binary))
    .digest("hex");
  evidence.environment = JSON.parse(
    await readFile(path.join(outputDir, "webdriver-environment.json"), "utf8"),
  );
  driver = spawn(
    nativeDriver,
    [
      "--port=4444",
      "--verbose",
      `--log-path=${path.join(outputDir, "msedgedriver.log")}`,
    ],
    {
      windowsHide: true,
      stdio: ["ignore", "pipe", "pipe"],
      env: {
        ...process.env,
        MSEDGEDRIVER_TELEMETRY_OPTOUT: "1",
        TAURI_AUTOMATION: "true",
        TAURI_WEBVIEW_AUTOMATION: "true",
      },
    },
  );
  evidence.driverPid = driver.pid;
  evidence.stage = "driver readiness";
  let driverError;
  driver.on("error", (error) => {
    driverError = error;
  });
  for (const stream of [driver.stdout, driver.stderr]) {
    stream.on("data", (chunk) => {
      driverLog = (driverLog + chunk.toString()).slice(-1_000_000);
    });
  }
  await waitUntil(
    async () => {
      if (driverError) throw driverError;
      return (await request("GET", "/status", undefined, 1000))?.ready;
    },
    20_000,
    "Microsoft EdgeDriver",
  );

  evidence.processesBeforeSession = await processSnapshot();
  evidence.stage = "WebView2 session creation";
  const session = await request(
    "POST",
    "/session",
    {
      capabilities: {
        alwaysMatch: {
          // These are tauri-driver 2.0.6's Windows capability translations.
          browserName: "webview2",
          "ms:edgeChromium": true,
          "ms:edgeOptions": {
            binary,
            args: [],
            webviewOptions: {
              browserExecutableFolder: webviewFolder,
              ...(process.env.TAURI_TEST_USER_DATA_FOLDER && {
                userDataFolder: process.env.TAURI_TEST_USER_DATA_FOLDER,
              }),
            },
          },
        },
      },
    },
    60_000,
  );
  sessionId = session.sessionId;
  evidence.capabilities = session.capabilities;
  evidence.stage = "release page load";
  await command("POST", "/timeouts", { script: 10_000, pageLoad: 30_000 });

  // Capture errors and CSP events before any script on the next load. CDP is
  // provided by the external test driver; no instrumentation enters app assets.
  await command("POST", "/ms/cdp/execute", {
    cmd: "Page.addScriptToEvaluateOnNewDocument",
    params: {
      source: `
        window.__scaffoldDiagnostics = { errors: [], violations: [] };
        window.addEventListener('error', event => {
          window.__scaffoldDiagnostics.errors.push({
            message: event.message || 'resource load failed',
            source: event.filename || event.target?.src || event.target?.href || ''
          });
        }, true);
        window.addEventListener('unhandledrejection', event => {
          window.__scaffoldDiagnostics.errors.push({message: String(event.reason)});
        });
        document.addEventListener('securitypolicyviolation', event => {
          window.__scaffoldDiagnostics.violations.push({
            effectiveDirective: event.effectiveDirective,
            blockedURI: event.blockedURI,
            disposition: event.disposition
          });
        });
      `,
    },
  });
  await command("POST", "/refresh", {});
  await waitUntil(
    () =>
      command("POST", "/execute/sync", {
        script:
          "return document.readyState === 'complete' && !!document.querySelector('main h1');",
        args: [],
      }),
    15_000,
    "the release placeholder",
  );
  const page = await command("POST", "/execute/sync", {
    script: `
      const main = document.querySelector('main');
      const rect = main.getBoundingClientRect();
      return {
        title: document.title,
        heading: main.querySelector('h1').textContent,
        display: getComputedStyle(main).display,
        width: rect.width, height: rect.height,
        lines: [...main.querySelectorAll('li')].map(item => item.textContent),
        diagnostics: window.__scaffoldDiagnostics
      };
    `,
    args: [],
  });
  evidence.page = page;
  assertPageLoaded(page);
  const screenshot = await command("GET", "/screenshot");
  await writeFile(
    path.join(outputDir, "release-placeholder.png"),
    Buffer.from(screenshot, "base64"),
  );

  evidence.stage = "core command denial";
  evidence.command = await command("POST", "/execute/async", {
    script: `
      const done = arguments[arguments.length - 1];
      window.__TAURI_INTERNALS__.invoke('plugin:app|version').then(
        value => done({status: 'resolved', value}),
        error => done({status: 'rejected', error: String(error)})
      );
    `,
    args: [],
  });
  assertCommandDenied(evidence.command);

  evidence.stage = "external request CSP";
  evidence.external = await command("POST", "/execute/async", {
    script: `
      const url = arguments[0];
      const done = arguments[arguments.length - 1];
      fetch(url, {signal: AbortSignal.timeout(3000)}).then(
        () => done({status: 'resolved', violations: []}),
        error => setTimeout(() => done({
          status: 'rejected', error: String(error),
          violations: window.__scaffoldDiagnostics.violations
        }), 50)
      );
    `,
    args: [externalProbeUrl],
  });
  assertExternalRequestBlocked(evidence.external);
  evidence.finalDiagnostics = await command("POST", "/execute/sync", {
    script: "return window.__scaffoldDiagnostics;",
    args: [],
  });
  assert.deepEqual(evidence.finalDiagnostics.errors, []);
  assert.ok(
    evidence.finalDiagnostics.violations.every(
      (event) =>
        event.effectiveDirective === "connect-src" &&
        [externalProbeUrl, new URL(externalProbeUrl).origin].includes(event.blockedURI),
    ),
    "Unexpected CSP violation after the probes",
  );
  evidence.passed = true;
  evidence.stage = "completed";
  console.log(
    "Native release smoke: placeholder, denied core API, and enforced external-request CSP passed.",
  );
} catch (error) {
  evidence.error = error.stack ?? String(error);
  evidence.cause = error.cause?.stack ?? String(error.cause ?? "");
  try {
    evidence.processesAtFailure = await processSnapshot();
  } catch (snapshotError) {
    evidence.processSnapshotError = String(snapshotError);
  }
  console.error(evidence.error);
  process.exitCode = 1;
} finally {
  if (sessionId) {
    try {
      await command("DELETE", "");
    } catch (error) {
      evidence.cleanupError = String(error);
      evidence.passed = false;
      process.exitCode = 1;
    }
  }
  if (driver) {
    evidence.driverExitCode = driver.exitCode;
    evidence.driverSignalCode = driver.signalCode;
    // Terminate only the process tree rooted at the driver started by this run.
    if (driver.pid && driver.exitCode === null) {
      try {
        await runFile("taskkill.exe", ["/PID", String(driver.pid), "/T", "/F"], {
          windowsHide: true,
          timeout: 10_000,
        });
      } catch (error) {
        evidence.driverCleanupError = String(error);
        evidence.passed = false;
        process.exitCode = 1;
      }
    }
  }
  await writeFile(
    path.join(outputDir, "native-smoke.json"),
    `${JSON.stringify(evidence, null, 2)}\n`,
  );
  await writeFile(path.join(outputDir, "driver-console.log"), driverLog);
}
