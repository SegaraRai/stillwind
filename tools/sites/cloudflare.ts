import { cp } from "node:fs/promises";
import path from "node:path";

import {
  cleanBuildOutputDir,
  readBuildOutput,
  writeAssets,
  writeRootConfig,
  writeWorkerConfig,
} from "@cloudflare/build-output-utils";
import { loadAndParseConfig } from "@cloudflare/config";

import { parseDeployment, parseSite, prepareSite, type Deployment, type Site } from "./prepare.ts";

/**
 * Copies Preview Build Output from source into an empty destination for the Worker's first deployment.
 * Keeps its assets and domains, and rejects builds with production routes.
 * The caller must remove the destination after deployment.
 */
export async function writeInitialBuild(source: string, destination: string): Promise<void> {
  const output = await readBuildOutput(source);
  const config = output.workers.default.config;
  if (
    !output.rootConfig.buildContext.isPreview ||
    config.workersDev !== false ||
    config.domains?.some((domain) => typeof domain === "string" || domain.production) ||
    config.triggers?.length
  ) {
    throw new Error("Initial Preview deployment requires a Worker with no production routes");
  }
  await cp(path.join(source, ".cloudflare"), path.join(destination, ".cloudflare"), {
    recursive: true,
  });
  await writeRootConfig(
    destination,
    {
      accountId: output.rootConfig.accountId,
      complianceRegion: output.rootConfig.complianceRegion,
    },
    { mode: output.rootConfig.buildContext.mode, isPreview: false },
  );
}

/**
 * Writes and validates Cloudflare Build Output for the prepared assets under .deploy/<site>.
 * Loads the application's configuration with the requested mode and Preview context.
 * Throws when the configuration or assets do not satisfy Cloudflare's Build Output schema.
 */
export async function writeSiteBuild(
  root: string,
  site: Site,
  deployment: Deployment,
): Promise<void> {
  const context = {
    mode: deployment,
    isPreview: deployment === "preview",
  };
  const { result } = await loadAndParseConfig(
    path.join(root, `apps/${site}/cloudflare.config.ts`),
    context,
  );
  if (!result.success) {
    throw result.error;
  }
  const { worker, accountId, complianceRegion } = result.data;
  if (!worker || worker.entrypoint) {
    throw new Error("Site deployments require an assets-only Worker");
  }
  const output = path.resolve(root, ".deploy", site);
  await cleanBuildOutputDir(output);
  await writeAssets({
    root: output,
    sourceDirectory: output,
  });
  await writeWorkerConfig({
    root: output,
    config: worker,
  });
  await writeRootConfig(
    output,
    {
      accountId,
      complianceRegion,
    },
    context,
  );
  await readBuildOutput(output);
}

if (import.meta.main) {
  const root = path.resolve(import.meta.dirname, "../..");
  const site = parseSite(process.argv[2]);
  const deployment = parseDeployment(process.env.SITE_DEPLOYMENT);
  await prepareSite(root, site, deployment);
  await writeSiteBuild(root, site, deployment);
}
