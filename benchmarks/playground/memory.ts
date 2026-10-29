import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { dirname, resolve } from "node:path";

import { chromium, type CDPSession, type Page } from "playwright";

import {
  buildProbe,
  editedApp,
  editSource,
  failNextBuild,
  installBuildProbe,
  presetSource,
  waitForBuild,
} from "../../e2e/play/src/browser.ts";
import { servePlayground } from "../../e2e/play/src/server.ts";

interface WasmMemory {
  role: string;
  initialBytes: number;
  currentBytes: number;
}

interface MemorySample {
  stage: string;
  privateBytes: number | null;
  workingSetBytes: number | null;
  browserProcesses: number;
  workerTargets: number;
  wasm: WasmMemory[];
}

interface MemoryRun {
  preset: string;
  iteration: number;
  hardwareConcurrency: number;
  samples: MemorySample[];
  recoverySucceeded?: boolean;
  recoveryError?: string;
}

const directory =
  process.env.STILLWIND_PLAY_TEST_DIRECTORY ??
  resolve(import.meta.dirname, "../../apps/play/dist/acceptance");
const output = resolve(
  process.env.STILLWIND_PLAY_MEMORY_OUTPUT ??
    resolve(import.meta.dirname, "../results/playground-memory.json"),
);
const repetitions = Number(process.env.STILLWIND_PLAY_MEMORY_RUNS ?? 3);
const selectedPresets = (
  process.env.STILLWIND_PLAY_MEMORY_PRESETS ?? "minimal,release-checklist"
).split(",");
if (
  !Number.isSafeInteger(repetitions) ||
  repetitions < 1 ||
  selectedPresets.some((preset) => !["minimal", "release-checklist"].includes(preset))
) {
  throw new Error("Memory measurements require a positive run count and known presets");
}
const report = {
  gitRevision: execFileSync("git", ["rev-parse", "HEAD"], { encoding: "utf8" }).trim(),
  nodeVersion: process.version,
  recordedAt: new Date().toISOString(),
  platform: process.platform,
  artifactHashes: Object.fromEntries(
    await Promise.all(
      [
        "index.html",
        "extensions/stillwind/dist/browser/stillwind_native.wasm32-wasi.wasm",
        "extensions/stillwind/dist/browser/rolldown-binding.wasm32-wasi.wasm",
      ].map(
        async (file) =>
          [
            file,
            createHash("sha256")
              .update(await readFile(resolve(directory, file)))
              .digest("hex"),
          ] as const,
      ),
    ),
  ),
  browserVersion: "",
  processMetric:
    "Windows PrivateMemorySize64 and WorkingSet64 summed across SystemInfo.getProcessInfo PIDs",
  garbageCollection:
    "Owner workers request gc() before reporting; Chrome starts with --js-flags=--expose-gc",
  runs: [] as MemoryRun[],
};

function instrumentMemory(source: string, file: string): string {
  if (!source.includes("new WebAssembly.Memory(")) {
    return source;
  }
  const role = file.includes("extensions")
    ? source.includes("stillwind_native.wasm32-wasi.wasm")
      ? "stillwind"
      : "rolldown-config"
    : "rolldown-application";
  const preamble = `
if (!globalThis.__stillwindMakeMemory) {
  const records = [];
  const channel = new BroadcastChannel('stillwind-memory-measurement');
  globalThis.__stillwindMakeMemory = (role, descriptor) => {
    const memory = new WebAssembly.Memory(descriptor);
    records.push({role, initialBytes: descriptor.initial * 65536, memory});
    return memory;
  };
  channel.onmessage = ({data}) => {
    if (data.kind === 'measure') {
      if (typeof globalThis.gc === 'function') globalThis.gc();
      channel.postMessage({kind: 'sample', id: data.id, memories: records.map(({role,initialBytes,memory}) => ({role,initialBytes,currentBytes:memory.buffer.byteLength}))});
    }
  };
}
`;
  return (
    preamble +
    source.replaceAll(
      "new WebAssembly.Memory(",
      `globalThis.__stillwindMakeMemory(${JSON.stringify(role)},`,
    )
  );
}

async function processMemory(cdp: CDPSession) {
  const { processInfo } = await cdp.send("SystemInfo.getProcessInfo");
  const ids = processInfo.map((item: { id: number }) => item.id);
  if (process.platform !== "win32") {
    return {
      privateBytes: null,
      workingSetBytes: null,
      browserProcesses: ids.length,
    };
  }
  const script = `$items = @(Get-Process -Id ${ids.join(",")} -ErrorAction SilentlyContinue); [pscustomobject]@{ privateBytes = [long](($items | Measure-Object -Property PrivateMemorySize64 -Sum).Sum); workingSetBytes = [long](($items | Measure-Object -Property WorkingSet64 -Sum).Sum); browserProcesses = $items.Count } | ConvertTo-Json -Compress`;
  return JSON.parse(
    execFileSync(
      "powershell.exe",
      [
        "-NoProfile",
        "-NonInteractive",
        "-EncodedCommand",
        Buffer.from(script, "utf16le").toString("base64"),
      ],
      {
        encoding: "utf8",
        windowsHide: true,
        stdio: ["ignore", "pipe", "pipe"],
      },
    ),
  ) as {
    privateBytes: number;
    workingSetBytes: number;
    browserProcesses: number;
  };
}

async function sample(page: Page, cdp: CDPSession, stage: string): Promise<MemorySample> {
  const wasm = await page.evaluate(async () => {
    const id = crypto.randomUUID();
    const channel = new BroadcastChannel("stillwind-memory-measurement");
    return new Promise<WasmMemory[]>((resolve, reject) => {
      const memories = new Map<string, WasmMemory>();
      const timer = setTimeout(() => {
        channel.close();
        reject(new Error(`Only ${memories.size} WASM memories reported`));
      }, 10_000);
      channel.onmessage = ({ data }) => {
        if (data.kind !== "sample" || data.id !== id) {
          return;
        }
        for (const memory of data.memories as WasmMemory[]) {
          memories.set(memory.role, memory);
        }
        if (memories.size === 3) {
          clearTimeout(timer);
          channel.close();
          resolve([...memories.values()].sort((a, b) => a.role.localeCompare(b.role)));
        }
      };
      channel.postMessage({
        kind: "measure",
        id,
      });
    });
  });
  const { targetInfos } = await cdp.send("Target.getTargets");
  const processes = await processMemory(cdp);
  return {
    stage,
    ...processes,
    workerTargets: targetInfos.filter((item: { type: string }) => item.type === "worker").length,
    wasm,
  };
}

const server = await servePlayground(directory, instrumentMemory);
await mkdir(dirname(output), { recursive: true });
let recoveryFailures = 0;
try {
  for (const preset of selectedPresets) {
    for (let iteration = 1; iteration <= repetitions; iteration++) {
      const browser = await chromium.launch({
        channel: "chrome",
        headless: true,
        args: ["--js-flags=--expose-gc"],
      });
      report.browserVersion = browser.version();
      const browserErrors: string[] = [];
      try {
        console.info(`Starting ${preset} run ${iteration}`);
        const page = await browser.newPage({
          viewport: {
            width: 1440,
            height: 900,
          },
        });
        page.on("pageerror", (error) => browserErrors.push(error.stack ?? error.message));
        page.on("console", (message) => {
          if (message.type() === "error") {
            browserErrors.push(message.text());
          }
        });
        page.on("requestfailed", (request) =>
          browserErrors.push(`${request.url()}: ${request.failure()?.errorText}`),
        );
        page.setDefaultTimeout(120_000);
        const cdp = await browser.newBrowserCDPSession();
        await installBuildProbe(page);
        await page.goto(`${server.origin}/#preset=${preset}`);
        await waitForBuild(page, 0);
        const main = page
          .frameLocator('iframe[title="Stillwind Playground Preview"]')
          .locator("main");
        await main.waitFor();
        const run: MemoryRun = {
          preset,
          iteration,
          hardwareConcurrency: await page.evaluate(() => navigator.hardwareConcurrency),
          samples: [],
        };
        const record = async (stage: string) => {
          const result = await sample(page, cdp, stage);
          run.samples.push(result);
          console.info(
            JSON.stringify({
              preset,
              iteration,
              ...result,
            }),
          );
        };
        await record("ready");
        const app = await presetSource(preset, "App.tsx");
        const css = await presetSource(preset, "stillwind.css");
        const config = await presetSource(preset, "stillwind.config.ts");
        for (let revision = 1; revision <= 5; revision++) {
          const before = await buildProbe(page);
          await editSource(page, "App.tsx", editedApp(app, revision));
          await waitForBuild(page, before.builds);
          await main.locator(`xpath=self::*[@data-build='${revision}']`).waitFor();
        }
        await record("source-edits");
        let before = await buildProbe(page);
        await editSource(
          page,
          "stillwind.css",
          `${css}\nmain { outline: 3px solid rgb(1, 2, 3); }\n`,
        );
        await waitForBuild(page, before.builds);
        await record("css-edit");
        before = await buildProbe(page);
        await editSource(
          page,
          "stillwind.config.ts",
          config
            .replace("defineConfig({", "defineConfig(async () => ({")
            .replace(/\}\);\s*$/, "}));\n"),
        );
        await waitForBuild(page, before.builds);
        await record("config-edit");
        before = await buildProbe(page);
        await failNextBuild(page);
        await editSource(page, "App.tsx", editedApp(app, 6));
        await waitForBuild(page, before.builds, "Build error");
        const failed = await buildProbe(page);
        await editSource(page, "App.tsx", editedApp(app, 7));
        try {
          await waitForBuild(page, failed.builds, "Ready", 30_000);
          await main.locator("xpath=self::*[@data-build='7']").waitFor({
            timeout: 30_000,
          });
          await record("worker-recovered");
          run.recoverySucceeded = true;
        } catch (error) {
          recoveryFailures++;
          run.recoverySucceeded = false;
          run.recoveryError = String(error);
          console.error(`Recovery failed: ${preset} run ${iteration}: ${String(error)}`);
        }
        report.runs.push(run);
        await mkdir(dirname(output), { recursive: true });
        await writeFile(output, JSON.stringify(report, null, 2) + "\n");
      } catch (error) {
        const page = browser.contexts().flatMap((context) => context.pages())[0];
        const failure = {
          preset,
          iteration,
          error: String(error),
          browserErrors,
          body: await page
            ?.locator("body")
            .innerText({ timeout: 5_000 })
            .catch(() => ""),
        };
        await writeFile(
          output.replace(/\.json$/, "-failure.json"),
          JSON.stringify(failure, null, 2),
        );
        await page
          ?.screenshot({
            path: output.replace(/\.json$/, "-failure.png"),
            timeout: 5_000,
          })
          .catch(() => {});
        throw error;
      } finally {
        await browser.close();
      }
    }
  }
} finally {
  await server.close();
}
if (recoveryFailures > 0) {
  process.exitCode = 1;
}
