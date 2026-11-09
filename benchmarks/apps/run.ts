/**
 * Measures generated React Router applications with Stillwind, with Tailwind CSS, and without
 * generated CSS.
 *
 * Each run measures a production build, development server startup, the first page
 * load, and edits until the page shows them. Results go to `results/apps.json`.
 */
import { execFileSync } from "node:child_process";
import { mkdir, mkdtemp, readdir, readFile, rm, writeFile } from "node:fs/promises";
import { createRequire } from "node:module";
import { dirname, join } from "node:path";
import { setTimeout as delay } from "node:timers/promises";
import { fileURLToPath, pathToFileURL } from "node:url";

import { availablePort } from "@stillwind-internal/e2e-harness/port";
import { startNode, waitForServer } from "@stillwind-internal/e2e-harness/process";
import { chromium, type Page } from "playwright";

import {
  appEdits,
  generateApp,
  type TailwindPaths,
  type AppEditName,
  type AppShape,
  type AppVariant,
} from "./generate.ts";

const benchmarks = fileURLToPath(new URL("../", import.meta.url));
const repository = join(benchmarks, "..");
/** The e2e fixture whose installed dependencies the generated applications resolve. */
const fixture = join(repository, "e2e/frameworks/vite8-react19-reactrouter8");
const preload = pathToFileURL(fileURLToPath(new URL("./preload.mjs", import.meta.url))).href;
const cli = join(
  dirname(createRequire(join(fixture, "package.json")).resolve("@react-router/dev/package.json")),
  "bin.cjs",
);

/** Tailwind CSS packages of this benchmark package, which the generated applications cannot resolve. */
const tailwind: TailwindPaths = {
  plugin: import.meta.resolve("@tailwindcss/vite"),
  stylesheet: fileURLToPath(import.meta.resolve("tailwindcss/index.css")).replaceAll("\\", "/"),
};

const shapes: Record<string, AppShape> = {
  small: { routes: 8, components: 24, chain: 3, componentsPerRoute: 3, barrel: true },
  medium: { routes: 30, components: 240, chain: 4, componentsPerRoute: 4, barrel: true },
  large: { routes: 100, components: 1200, chain: 6, componentsPerRoute: 5, barrel: true },
};

const sizes = list("STILLWIND_BENCH_APPS_SIZES", "small,medium");
const variants = list("STILLWIND_BENCH_APPS_VARIANTS", "plain,tailwind,stillwind") as AppVariant[];
const runs = Number(process.env.STILLWIND_BENCH_APPS_RUNS ?? 3);
const keep = process.env.STILLWIND_BENCH_APPS_KEEP === "1";
const output = process.env.STILLWIND_BENCH_APPS_OUTPUT ?? join(benchmarks, "results/apps.json");

/** Totals of one phase across the processes of a measured command. */
interface PhaseTotal {
  calls: number;
  items: number;
  /** Sum of phase durations in milliseconds. Concurrent phases overlap. */
  total: number;
  /** Longest single phase in milliseconds. */
  max: number;
}

/** Resource usage and phases of a measured command. */
interface ProcessStats {
  /** Largest resident set size of one process, in kilobytes. */
  maxRssKb: number;
  /** User and system CPU time summed over all Node processes, in milliseconds. */
  cpuMs: number;
  phases: Record<string, PhaseTotal>;
}

interface RunResult {
  buildMs: number;
  build: ProcessStats;
  /** Milliseconds from spawning the development server to its first successful SSR response. */
  devReadyMs: number;
  /** Milliseconds from navigation to hydration in a fresh browser page. */
  firstLoadMs: number;
  /** Milliseconds from navigation to hydration after the server has transformed the page. */
  reloadMs: number;
  /** Milliseconds from the file write until the page shows each edit. */
  edits: Partial<Record<AppEditName, number>>;
  /** Phases recorded while each edit and its restoration were processed. */
  editPhases: Partial<Record<AppEditName, Record<string, PhaseTotal>>>;
  dev: ProcessStats;
}

function list(name: string, fallback: string): string[] {
  return (process.env[name] ?? fallback)
    .split(",")
    .map((value) => value.trim())
    .filter(Boolean);
}

async function readStats(directory: string): Promise<ProcessStats> {
  const stats: ProcessStats = { maxRssKb: 0, cpuMs: 0, phases: {} };
  for (const name of await readdir(directory)) {
    const entry = JSON.parse(await readFile(join(directory, name), "utf8"));
    stats.maxRssKb = Math.max(stats.maxRssKb, entry.maxRssKb);
    stats.cpuMs += entry.cpuUserMs + entry.cpuSystemMs;
    for (const [phase, value] of Object.entries(entry.phases as Record<string, PhaseTotal>)) {
      const total = (stats.phases[phase] ??= { calls: 0, items: 0, total: 0, max: 0 });
      total.calls += value.calls;
      total.items += value.items;
      total.total += value.total;
      total.max = Math.max(total.max, value.max);
    }
  }
  return stats;
}

function phaseDifference(
  after: Record<string, PhaseTotal>,
  before: Record<string, PhaseTotal>,
): Record<string, PhaseTotal> {
  const difference: Record<string, PhaseTotal> = {};
  for (const [phase, value] of Object.entries(after)) {
    const previous = before[phase] ?? { calls: 0, items: 0, total: 0, max: 0 };
    if (value.calls > previous.calls) {
      difference[phase] = {
        calls: value.calls - previous.calls,
        items: value.items - previous.items,
        total: value.total - previous.total,
        // A maximum cannot be separated by subtraction, so this is the maximum so far.
        max: value.max,
      };
    }
  }
  return difference;
}

async function freshDirectory(path: string) {
  await rm(path, { recursive: true, force: true, maxRetries: 10, retryDelay: 100 });
  await mkdir(path, { recursive: true });
  return path;
}

function measuredNode(args: string[], cwd: string, traceDirectory: string) {
  return startNode(
    [cli, ...args],
    cwd,
    {
      // React Router's development command restarts Node without the parent's argv flags.
      NODE_OPTIONS: `${process.env.NODE_OPTIONS ?? ""} --import=${preload}`.trim(),
      STILLWIND_BENCH_TRACE_DIR: traceDirectory,
      NODE_ENV: args[0] === "build" ? "production" : "development",
    },
    1_200_000,
  );
}

async function build(root: string): Promise<Pick<RunResult, "buildMs" | "build">> {
  await rm(join(root, "build"), { recursive: true, force: true });
  await rm(join(root, "node_modules/.vite"), { recursive: true, force: true });
  const traces = await freshDirectory(`${root}.trace-build`);
  const start = performance.now();
  const child = measuredNode(["build"], root, traces);
  const code = await child.done;
  const buildMs = performance.now() - start;
  if (code !== 0) {
    throw new Error(`Build failed with ${code}\n${child.logs().slice(-10_000)}`);
  }
  return { buildMs, build: await readStats(traces) };
}

/** Polls the page because an edit can reload the document and destroy evaluation contexts. */
async function waitForPage(
  check: () => Promise<boolean>,
  describe: () => Promise<string>,
  timeout = 60_000,
) {
  const deadline = Date.now() + timeout;
  while (Date.now() < deadline) {
    try {
      if (await check()) {
        return;
      }
    } catch {
      // The document is navigating.
    }
    await delay(5);
  }
  throw new Error(`The page did not show the expected state: ${await describe().catch(String)}`);
}

async function hydrate(page: Page, url: string): Promise<number> {
  const start = performance.now();
  await page.goto(url);
  await page.waitForSelector('html[data-hydrated="yes"]', { timeout: 120_000 });
  return performance.now() - start;
}

async function edit(page: Page, root: string, variant: AppVariant, name: AppEditName) {
  const { file, from, to, selector, property, value } = appEdits[name];
  const path = join(root, file);
  const original = await readFile(path, "utf8");
  const read = () =>
    page.evaluate(
      ({ selector, property }) => {
        const element = document.querySelector(selector);
        return element
          ? {
              className: element.className,
              value: getComputedStyle(element).getPropertyValue(property),
            }
          : null;
      },
      { selector, property },
    );
  const before = await read();
  if (!before) {
    throw new Error(`Missing ${selector}`);
  }
  const shown = async () => {
    const current = await read();
    return variant !== "plain"
      ? current?.value === value
      : current !== null && current.className !== before.className;
  };
  const describe = async () =>
    `${name} edit, before ${JSON.stringify(before)}, current ${JSON.stringify(await read())}`;
  const start = performance.now();
  await writeFile(path, original.replace(from, to));
  await waitForPage(shown, describe);
  const elapsed = performance.now() - start;
  // Let the watcher settle so that it does not merge the restoration with the edit.
  await delay(500);
  await writeFile(path, original);
  await waitForPage(async () => {
    const current = await read();
    return current?.className === before.className && current.value === before.value;
  }, describe);
  await delay(500);
  return elapsed;
}

async function develop(
  root: string,
  variant: AppVariant,
): Promise<Omit<RunResult, "buildMs" | "build">> {
  await rm(join(root, "node_modules/.vite"), { recursive: true, force: true });
  const traces = await freshDirectory(`${root}.trace-dev`);
  const port = await availablePort();
  const url = `http://localhost:${port}/`;
  const start = performance.now();
  const child = measuredNode(["dev", "--port", String(port), "--strictPort"], root, traces);
  const browser = await chromium.launch({ channel: "chrome", headless: true });
  try {
    await waitForServer(url, child, 600_000);
    const devReadyMs = performance.now() - start;
    const page = await browser.newPage();
    const firstLoadMs = await hydrate(page, url);
    const reloadMs = await hydrate(page, url);
    const edits: RunResult["edits"] = {};
    const editPhases: RunResult["editPhases"] = {};
    for (const name of Object.keys(appEdits) as AppEditName[]) {
      if (name === "theme" && variant === "plain") {
        continue;
      }
      // Each wait covers the preload's periodic trace write.
      await delay(1000);
      const before = await readStats(traces);
      edits[name] = await edit(page, root, variant, name);
      await delay(1000);
      editPhases[name] = phaseDifference((await readStats(traces)).phases, before.phases);
    }
    return { devReadyMs, firstLoadMs, reloadMs, edits, editPhases, dev: await readStats(traces) };
  } finally {
    await browser.close();
    await child.stop();
  }
}

async function writeApp(root: string, variant: AppVariant, shape: AppShape) {
  for (const [file, contents] of generateApp(variant, shape, tailwind)) {
    await mkdir(dirname(join(root, file)), { recursive: true });
    await writeFile(join(root, file), contents);
  }
}

function median(values: number[]): number {
  const sorted = values.toSorted((a, b) => a - b);
  const middle = Math.floor(sorted.length / 2);
  return sorted.length % 2 ? sorted[middle] : (sorted[middle - 1] + sorted[middle]) / 2;
}

const work = join(fixture, ".e2e");
await mkdir(work, { recursive: true });
const report: {
  environment: Record<string, string>;
  settings: Record<string, unknown>;
  results: { size: string; shape: AppShape; variant: AppVariant; runs: RunResult[] }[];
} = {
  environment: {
    timestamp: new Date().toISOString(),
    revision: execFileSync("git", ["rev-parse", "HEAD"], {
      cwd: repository,
      encoding: "utf8",
    }).trim(),
    node: process.version,
    platform: process.platform,
    arch: process.arch,
  },
  settings: { sizes, variants, runs },
  results: [],
};

for (const size of sizes) {
  const shape = shapes[size];
  if (!shape) {
    throw new Error(`Unknown size ${size}; expected one of ${Object.keys(shapes).join(", ")}`);
  }
  for (const variant of variants) {
    const root = await mkdtemp(join(work, `bench-${size}-${variant}-`));
    try {
      await writeApp(root, variant, shape);
      const results: RunResult[] = [];
      for (let run = 0; run < runs; run++) {
        const result = { ...(await build(root)), ...(await develop(root, variant)) };
        results.push(result);
        console.log(
          `${size} ${variant} #${run + 1}: build ${result.buildMs.toFixed(0)}ms, ` +
            `dev ${result.devReadyMs.toFixed(0)}ms, load ${result.firstLoadMs.toFixed(0)}ms, ` +
            `edits ${JSON.stringify(Object.fromEntries(Object.entries(result.edits).map(([k, v]) => [k, Math.round(v)])))}`,
        );
      }
      report.results.push({ size, shape, variant, runs: results });
    } finally {
      if (!keep) {
        // Traces live beside the application so that the development watcher ignores them.
        for (const path of [root, `${root}.trace-build`, `${root}.trace-dev`]) {
          await rm(path, { recursive: true, force: true, maxRetries: 10, retryDelay: 100 });
        }
      }
    }
  }
}

await mkdir(dirname(output), { recursive: true });
await writeFile(output, JSON.stringify(report, null, 2) + "\n");

const metrics: [string, (run: RunResult) => number | undefined][] = [
  ["build ms", (run) => run.buildMs],
  ["build CPU ms", (run) => run.build.cpuMs],
  ["build max RSS MB", (run) => run.build.maxRssKb / 1024],
  ["dev ready ms", (run) => run.devReadyMs],
  ["first load ms", (run) => run.firstLoadMs],
  ["reload ms", (run) => run.reloadMs],
  ["leaf edit ms", (run) => run.edits.leaf],
  ["shared edit ms", (run) => run.edits.shared],
  ["theme edit ms", (run) => run.edits.theme],
];
for (const size of sizes) {
  const rows: Record<string, Record<string, string>> = {};
  for (const [label, read] of metrics) {
    const row: Record<string, string> = {};
    const medians: Partial<Record<AppVariant, number>> = {};
    for (const entry of report.results.filter((result) => result.size === size)) {
      const values = entry.runs.map(read).filter((value) => value !== undefined);
      if (values.length > 0) {
        medians[entry.variant] = median(values);
        row[entry.variant] = medians[entry.variant]!.toFixed(0);
      }
    }
    if (medians.plain && medians.stillwind) {
      row["stillwind / plain"] = `${(medians.stillwind / medians.plain).toFixed(2)}x`;
    }
    if (medians.tailwind && medians.stillwind) {
      row["stillwind / tailwind"] = `${(medians.stillwind / medians.tailwind).toFixed(2)}x`;
    }
    rows[label] = row;
  }
  console.log(`\n${size} (medians of ${runs} runs)`);
  console.table(rows);
  const stillwind = report.results.find(
    (result) => result.size === size && result.variant === "stillwind",
  );
  if (stillwind) {
    const last = stillwind.runs.at(-1)!;
    for (const [label, phases] of [
      ["build", last.build.phases],
      ["dev", last.dev.phases],
      ...Object.entries(last.editPhases).map(([name, value]) => [`${name} edit`, value] as const),
    ] as const) {
      console.log(`Stillwind ${label} phases (last run)`);
      console.table(
        Object.fromEntries(
          Object.entries(phases).map(([phase, value]) => [
            phase,
            {
              calls: value.calls,
              items: value.items,
              "total ms": Math.round(value.total),
              "max ms": Math.round(value.max),
            },
          ]),
        ),
      );
    }
  }
}
console.log(`\nReport: ${output}`);
