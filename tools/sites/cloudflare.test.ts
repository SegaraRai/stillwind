import { cp, mkdir, mkdtemp, readFile, rm, symlink, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";

import { readBuildOutput } from "@cloudflare/build-output-utils";
import { afterEach, expect, test } from "vite-plus/test";

import { writeInitialBuild, writePullRequestBuild, writeSiteBuild } from "./cloudflare.ts";
import { prepareSite } from "./prepare.ts";

const roots: string[] = [];
afterEach(async () => {
  await Promise.all(roots.splice(0).map((root) => rm(root, { recursive: true, force: true })));
});

async function pullRequestArtifact(): Promise<{
  root: string;
  artifact: string;
  assets: string;
}> {
  const root = await mkdtemp(path.join(os.tmpdir(), "stillwind-sites-pull-"));
  roots.push(root);
  await mkdir(path.join(root, "apps/website"), { recursive: true });
  await writeFile(
    path.join(root, "apps/website/cloudflare.config.ts"),
    `export default () => ({ worker: {
      name: "stillwind-website", compatibilityDate: "2026-10-29", workersDev: false,
    } });`,
  );
  const artifact = path.join(root, "artifact");
  const output = path.join(artifact, ".cloudflare/output/v0");
  const assets = path.join(output, "workers/default/assets");
  await mkdir(assets, { recursive: true });
  await writeFile(path.join(assets, "index.html"), "<title>Pull request</title>");
  await writeFile(path.join(assets, "_headers"), "/*\n  X-Robots-Tag: all\n");
  await writeFile(
    path.join(output, "workers/default/config.json"),
    '{"name":"stillwind-website","domains":["stillwind.dev"]}',
  );
  await writeFile(path.join(output, "config.json"), '{"buildContext":{"isPreview":false}}');
  return {
    root,
    artifact,
    assets,
  };
}

test("pull request builds keep their assets and use the trusted Worker configuration", async () => {
  const { root, artifact } = await pullRequestArtifact();
  await writePullRequestBuild(root, "website", artifact);
  const output = await readBuildOutput(path.join(root, ".deploy/website"));
  expect(output.rootConfig.buildContext).toEqual({
    mode: "preview",
    isPreview: true,
  });
  expect(output.workers.default.config.name).toBe("stillwind-website");
  expect(output.workers.default.config.domains).toBeUndefined();
  const assets = output.workers.default.assetsDir!;
  expect(await readFile(path.join(assets, "index.html"), "utf8")).toContain("Pull request");
  expect(await readFile(path.join(assets, "_headers"), "utf8")).toMatch(/X-Robots-Tag: noindex/);
  expect(await readFile(path.join(assets, "robots.txt"), "utf8")).toContain("Disallow: /");
});

test.runIf(process.platform !== "win32")(
  "pull request builds reject symlinks in the artifact",
  async () => {
    const nested = await pullRequestArtifact();
    await symlink("/etc/hostname", path.join(nested.assets, "secret.txt"));
    await expect(writePullRequestBuild(nested.root, "website", nested.artifact)).rejects.toThrow(
      "regular files",
    );
    const linked = await pullRequestArtifact();
    await rm(linked.assets, { recursive: true });
    await symlink(linked.root, linked.assets);
    await expect(writePullRequestBuild(linked.root, "website", linked.artifact)).rejects.toThrow(
      "through a symlink",
    );
  },
);

test("pull request builds require static assets", async () => {
  const { root, artifact, assets } = await pullRequestArtifact();
  await rm(assets, { recursive: true });
  await expect(writePullRequestBuild(root, "website", artifact)).rejects.toThrow("ENOENT");
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
        workersDev: false,
        domains: [
          ...(isPreview ? [] : ["stillwind.dev"]),
          {
            hostname: "preview.stillwind.dev",
            production: false,
            previews: true,
          },
        ]
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
    expect(output.workers.default.config.domains).toEqual([
      ...(deployment === "preview" ? [] : ["stillwind.dev"]),
      {
        hostname: "preview.stillwind.dev",
        production: false,
        previews: true,
      },
    ]);
    expect(output.workers.default.config.workersDev).toBe(false);
    expect(output.workers.default.bundleDir).toBeUndefined();
    const assets = output.workers.default.assetsDir;
    if (!assets) {
      expect.fail("Missing static assets");
    }
    expect(await readFile(path.join(assets, "index.html"), "utf8")).toContain("Stillwind");
    const initial = path.join(root, "initial");
    if (deployment === "preview") {
      await writeInitialBuild(restored, initial);
      const build = await readBuildOutput(initial);
      expect(build.rootConfig.buildContext).toEqual({ mode: "preview", isPreview: false });
      expect(build.workers.default.config).toEqual(output.workers.default.config);
      expect(
        await readFile(path.join(build.workers.default.assetsDir!, "index.html"), "utf8"),
      ).toContain("Stillwind");
    } else {
      await expect(writeInitialBuild(restored, initial)).rejects.toThrow("no production routes");
    }
  },
);
