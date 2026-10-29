import { cp, mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";

import { readBuildOutput } from "@cloudflare/build-output-utils";
import { afterEach, expect, test } from "vite-plus/test";

import { writeSiteBuild } from "./cloudflare.ts";
import { prepareSite } from "./prepare.ts";

const roots: string[] = [];
afterEach(async () => {
  await Promise.all(roots.splice(0).map((root) => rm(root, { recursive: true, force: true })));
});

test.each(["preview", "production"] as const)(
  "%s builds remain deployable after moving to another checkout",
  async (deployment) => {
    const root = await mkdtemp(path.join(os.tmpdir(), "stillwind-sites-build-"));
    roots.push(root);
    const app = path.join(root, "apps/website");
    await mkdir(path.join(app, "dist"), { recursive: true });
    await writeFile(path.join(app, "dist/index.html"), "<!doctype html><title>Stillwind</title>");
    await writeFile(
      path.join(app, "cloudflare.config.ts"),
      `export default ({ isPreview }) => ({ worker: {
        name: "stillwind-website", compatibilityDate: "2026-10-29",
        domains: isPreview ? [] : ["stillwind.dev"]
      } });`,
    );
    await prepareSite(root, "website", deployment);
    await writeSiteBuild(root, "website", deployment);
    const restored = path.join(root, "restored");
    await cp(path.join(root, ".deploy/website/.cloudflare"), path.join(restored, ".cloudflare"), {
      recursive: true,
    });
    const output = await readBuildOutput(restored);
    expect(output.rootConfig.buildContext).toEqual({
      mode: deployment,
      isPreview: deployment === "preview",
    });
    expect(output.workers.default.config.name).toBe("stillwind-website");
    expect(output.workers.default.config.domains).toEqual(
      deployment === "preview" ? [] : ["stillwind.dev"],
    );
    expect(output.workers.default.bundleDir).toBeUndefined();
    const assets = output.workers.default.assetsDir;
    if (!assets) {
      expect.fail("Missing static assets");
    }
    expect(await readFile(path.join(assets, "index.html"), "utf8")).toContain("Stillwind");
  },
);
