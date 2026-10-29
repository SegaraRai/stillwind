import { spawn } from "node:child_process";
import { appendFile } from "node:fs/promises";
import { createRequire } from "node:module";
import path from "node:path";
import { setTimeout } from "node:timers/promises";

import { checkPageUrls } from "./check-urls.ts";
import { parseSite, sites, type Deployment, type Site } from "./prepare.ts";

/**
 * Maps the two deployment branches to their destination. Throws for other refs.
 */
export function deploymentForBranch(branch: string | undefined): Deployment {
  if (branch === "main") {
    return "preview";
  }
  if (branch === "stable") {
    return "production";
  }
  throw new Error("Site deployments require the main or stable branch");
}

/**
 * Returns the branch Preview URL from cf's JSON output. Throws when no HTTPS URL was issued.
 */
export function previewUrl(output: string): string {
  const entry = JSON.parse(output) as {
    type?: string;
    preview_urls?: string[];
  };
  if (entry.type === "preview" && entry.preview_urls?.[0]) {
    const url = new URL(entry.preview_urls[0]);
    if (url.protocol === "https:") {
      return url.href;
    }
  }
  throw new Error("cf returned no HTTPS Preview URL; enable workers.dev Preview URLs");
}

/**
 * Checks the deployed site's routes, metadata, Preview indexing policy, and WASM response headers.
 * Throws when an HTTP request fails or a response breaks the hosting contract.
 */
export async function checkSite(site: Site, origin: string, deployment: Deployment): Promise<void> {
  const paths =
    site === "website"
      ? ["/", "/docs/", "/docs/index.md", "/llms.txt", "/docs/utilities/"]
      : [
          "/",
          "/workspace.json",
          "/extensions/stillwind/dist/browser/stillwind_native.wasm32-wasi.wasm",
        ];
  for (const route of paths) {
    const url = new URL(route, origin);
    const response = await fetch(url, {
      method: "HEAD",
      signal: AbortSignal.timeout(15_000),
    });
    if (!response.ok) {
      throw new Error(`${url}: HTTP ${response.status}`);
    }
    if (deployment === "preview" && !response.headers.get("x-robots-tag")?.includes("noindex")) {
      throw new Error(`${url}: missing Preview noindex header`);
    }
    if (deployment === "production" && response.headers.get("x-robots-tag")?.includes("noindex")) {
      throw new Error(`${url}: production response is excluded from indexing`);
    }
    if ((site === "play" && route === "/") || route === "/docs/utilities/") {
      if (
        response.headers.get("cross-origin-opener-policy") !== "same-origin" ||
        response.headers.get("cross-origin-embedder-policy") !== "require-corp"
      ) {
        throw new Error(`${url}: missing WASM isolation headers`);
      }
    }
    if (route.endsWith(".wasm") && response.headers.get("content-type") !== "application/wasm") {
      throw new Error(`${url}: expected application/wasm`);
    }
  }
  const pageUrl = new URL("/", origin);
  const page = await fetch(pageUrl, { signal: AbortSignal.timeout(15_000) });
  if (!page.ok) {
    throw new Error(`${pageUrl}: HTTP ${page.status}`);
  }
  checkPageUrls(await page.text(), pageUrl.href, new URL("og.png", pageUrl).href);
}

async function deploy(site: Site, branch: string | undefined, dryRun: boolean): Promise<void> {
  const deployment = deploymentForBranch(branch);
  if (dryRun && deployment === "preview") {
    throw new Error(
      "cf previews deploy has no dry run; Preview builds are validated with their schema",
    );
  }
  if (!dryRun && (!process.env.CLOUDFLARE_ACCOUNT_ID || !process.env.CLOUDFLARE_API_TOKEN)) {
    throw new Error("Set CLOUDFLARE_ACCOUNT_ID and CLOUDFLARE_API_TOKEN");
  }
  const root = path.resolve(import.meta.dirname, "../..");
  const require = createRequire(import.meta.url);
  const cli = path.join(path.dirname(require.resolve("cf/package.json")), "bin/cf");
  const args = [
    ...(deployment === "preview" ? ["previews", "deploy", "main"] : ["deploy"]),
    "--prebuilt",
    "--mode",
    deployment,
    ...(dryRun ? ["--dry-run"] : []),
  ];
  const output = await new Promise<string>((resolve, reject) => {
    const child = spawn(process.execPath, [cli, ...args], {
      cwd: path.join(root, ".deploy", site),
      stdio: ["ignore", "pipe", "inherit"],
      env: {
        ...process.env,
        CF_SEND_TELEMETRY: "false",
      },
    });
    let stdout = "";
    child.stdout.setEncoding("utf8");
    child.stdout.on("data", (chunk: string) => {
      stdout += chunk;
      process.stdout.write(chunk);
    });
    child.on("error", reject);
    child.on("exit", (code) => {
      if (code === 0) {
        resolve(stdout);
      } else {
        reject(new Error(`cf exited with code ${code}`));
      }
    });
  });
  if (dryRun) {
    return;
  }
  const url = deployment === "preview" ? previewUrl(output) : new URL(sites[site].origin).href;
  if (process.env.GITHUB_OUTPUT) {
    await appendFile(process.env.GITHUB_OUTPUT, `url=${url}\n`);
  }
  if (process.env.GITHUB_STEP_SUMMARY) {
    await appendFile(
      process.env.GITHUB_STEP_SUMMARY,
      `- ${site} (${deployment}): [${url}](${url}) — ${process.env.GITHUB_SHA ?? "local"}\n`,
    );
  }
  for (let attempt = 0; ; attempt++) {
    try {
      await checkSite(site, url, deployment);
      break;
    } catch (error) {
      if (attempt === 5) {
        throw error;
      }
      console.warn(`Waiting for deployment: ${String(error)}`);
      await setTimeout(5_000);
    }
  }
  console.log(`Verified ${site}: ${url}`);
}

if (import.meta.main) {
  const args = process.argv.slice(process.argv[2] === "--" ? 3 : 2);
  if (args.length < 2 || args.length > 3 || (args[2] && args[2] !== "--dry-run")) {
    throw new Error("Usage: deploy <website|play> <main|stable> [--dry-run]");
  }
  await deploy(parseSite(args[0]), args[1], args[2] === "--dry-run");
}
