import { appendFile, readFile, readdir } from "node:fs/promises";
import path from "node:path";

import {
  changedFiles,
  readWorkspacePackages,
  workspaceImpact,
  type WorkspacePackage,
} from "../workspace/impact.ts";
import { previewOrigin, type Site } from "./prepare.ts";

export const browserSuites = [
  "language",
  "diagnostics",
  "hmr",
  "sharing",
  "files",
  "recovery",
] as const;
type BrowserSuite = (typeof browserSuites)[number];
export interface SitePlan {
  sites: Site[];
  browser: BrowserSuite[];
  deploy: Site[];
  full: boolean;
  reason?: string;
  affectedPackages: string[];
}

const sourcePrefixes = {
  "crates/": ["@stillwind/native"],
  "tools/tailwind/": ["@stillwind/native"],
  "tools/webref/": ["@stillwind/native"],
};

function developerDocument(file: string): boolean {
  return file === "docs/TODO.md" || file === "DEVELOPMENT.md" || file.endsWith("/DEVELOPMENT.md");
}

/** Selects site checks through package consumers and E2E source ownership. */
export function planSites(
  packages: readonly WorkspacePackage[],
  files: readonly string[] | null,
): SitePlan {
  const inputs = files?.filter((file) => !developerDocument(file)) ?? null;
  const impact = workspaceImpact(packages, inputs, sourcePrefixes);
  const selectSites = (affected: readonly string[], full: boolean): Site[] =>
    (["website", "play"] as const).filter(
      (site) =>
        full ||
        affected.includes(`@stillwind-internal/apps-${site}`) ||
        affected.includes("@stillwind-internal/tools-sites"),
    );
  const sites = selectSites(impact.affectedPackages, impact.full);
  const selectedBrowser = new Set<BrowserSuite>();
  if (impact.full || sites.includes("play")) {
    browserSuites.forEach((suite) => selectedBrowser.add(suite));
  }
  for (const file of inputs ?? []) {
    if (file.startsWith("e2e/play/")) {
      const suite = browserSuites.find((name) => file === `e2e/play/src/${name}.test.ts`);
      if (suite) {
        selectedBrowser.add(suite);
      } else {
        browserSuites.forEach((name) => selectedBrowser.add(name));
      }
    }
  }
  if (selectedBrowser.size && !sites.includes("play")) {
    sites.push("play");
  }
  const publicationInputs =
    inputs?.filter(
      (file) => !file.startsWith("e2e/") && !/\.(?:test|spec)\.[cm]?[jt]sx?$/.test(file),
    ) ?? null;
  const publication = workspaceImpact(packages, publicationInputs, sourcePrefixes);
  return {
    sites,
    browser: browserSuites.filter((suite) => selectedBrowser.has(suite)),
    deploy: selectSites(publication.affectedPackages, publication.full),
    full: impact.full,
    reason: impact.reason,
    affectedPackages: impact.affectedPackages,
  };
}

/**
 * Selects the origin embedded in each site's build.
 * A pull request build uses its own Preview origin for each planned site, so one build can be
 * published without rebuilding. An unplanned site has no pull request Preview, so links to it
 * keep the configured origin.
 */
export function buildUrls(
  plannedSites: readonly Site[],
  pull: number | undefined,
  configured: Record<Site, string>,
): Record<Site, string> {
  const select = (site: Site) =>
    pull !== undefined && plannedSites.includes(site)
      ? previewOrigin(site, `pr-${pull}`)
      : configured[site];
  return { website: select("website"), play: select("play") };
}

/** Rejects a browser matrix that would omit a test file or execute a missing scenario. */
export async function checkBrowserSuites(directory: string): Promise<void> {
  const files = (await readdir(directory, { recursive: true }))
    .filter((file) => file.endsWith(".test.ts"))
    .map((file) => file.split(path.sep).join("/"))
    .sort();
  const expected = browserSuites.map((suite) => `${suite}.test.ts`).sort();
  if (JSON.stringify(files) !== JSON.stringify(expected)) {
    throw new Error(
      "Update browserSuites to cover every playground browser test: " + files.join(", "),
    );
  }
}

type JobResult = {
  result: string;
  outputs?: Record<string, string>;
};

/** Requires every selected job to succeed and every unselected job to be skipped. */
export function requireSiteChecks(needs: Record<string, JobResult>): void {
  if (needs.changes?.result !== "success") {
    throw new Error("Change detection did not succeed");
  }
  const plan: SitePlan = JSON.parse(needs.changes.outputs?.plan ?? "null");
  const selection = (value: unknown, allowed: readonly string[]): value is string[] =>
    Array.isArray(value) &&
    value.every((entry) => allowed.includes(entry)) &&
    new Set(value).size === value.length;
  if (
    !plan ||
    !selection(plan.sites, ["website", "play"]) ||
    !selection(plan.browser, browserSuites) ||
    !selection(plan.deploy, ["website", "play"]) ||
    typeof plan.full !== "boolean" ||
    (plan.browser.length > 0 && !plan.sites.includes("play")) ||
    plan.deploy.some((site) => !plan.sites.includes(site))
  ) {
    throw new Error("Missing CI plan");
  }
  for (const [job, required] of [
    ["build", plan.sites.length > 0],
    ["browser", plan.browser.length > 0],
  ] as const) {
    const expected = required ? "success" : "skipped";
    if (needs[job]?.result !== expected) {
      throw new Error(`${job}: expected ${expected}, received ${needs[job]?.result}`);
    }
  }
}

if (import.meta.main) {
  if (process.argv[2] === "verify") {
    requireSiteChecks(JSON.parse(process.env.CI_NEEDS ?? "{}"));
  } else {
    const root = path.resolve(import.meta.dirname, "../..");
    await checkBrowserSuites(path.join(root, "e2e/play/src"));
    const listing = JSON.parse(await readFile(process.argv[2]!, "utf8"));
    const packages = await readWorkspacePackages(root, listing);
    const event = JSON.parse(await readFile(process.env.GITHUB_EVENT_PATH!, "utf8"));
    const files = changedFiles(
      root,
      process.env.GITHUB_EVENT_NAME ?? "",
      event,
      process.env.SITES_BASE_SHA,
    );
    const plan = planSites(packages, files);
    console.log(JSON.stringify(plan, null, 2));
    if (process.env.GITHUB_OUTPUT) {
      const urls = buildUrls(
        plan.sites,
        process.env.GITHUB_EVENT_NAME === "pull_request" ? event.number : undefined,
        {
          website: process.env.PUBLIC_WEBSITE_URL ?? "",
          play: process.env.PUBLIC_PLAYGROUND_URL ?? "",
        },
      );
      const outputs = {
        plan: JSON.stringify(plan),
        website_url: urls.website,
        play_url: urls.play,
        build: String(plan.sites.length > 0),
        website: String(plan.sites.includes("website")),
        play: String(plan.sites.includes("play")),
        browser: JSON.stringify(plan.browser),
        browser_required: String(plan.browser.length > 0),
        deploy: JSON.stringify(plan.deploy),
        deploy_required: String(plan.deploy.length > 0),
      };
      await appendFile(
        process.env.GITHUB_OUTPUT,
        Object.entries(outputs)
          .map(([name, value]) => `${name}=${value}\n`)
          .join(""),
      );
    }
    if (process.env.GITHUB_STEP_SUMMARY) {
      await appendFile(
        process.env.GITHUB_STEP_SUMMARY,
        `### Selected site checks\n\n\`\`\`json\n${JSON.stringify(plan, null, 2)}\n\`\`\`\n`,
      );
    }
  }
}
