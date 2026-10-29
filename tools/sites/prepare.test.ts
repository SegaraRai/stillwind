import { mkdtemp, mkdir, open, readFile, readdir, rm, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";

import { afterEach, expect, test } from "vite-plus/test";

import { prepareSite } from "./prepare.ts";

const roots: string[] = [];

afterEach(async () => {
  await Promise.all(roots.splice(0).map((root) => rm(root, { recursive: true, force: true })));
});

async function fixture() {
  const root = await mkdtemp(path.join(os.tmpdir(), "stillwind-sites-"));
  roots.push(root);
  const source = path.join(root, "apps/play/dist/site");
  await mkdir(source, { recursive: true });
  await writeFile(path.join(source, "index.html"), "<!doctype html><title>Playground</title>");
  await writeFile(
    path.join(source, "_headers"),
    "/*\n  Cross-Origin-Opener-Policy: same-origin\n  Cross-Origin-Embedder-Policy: require-corp\n",
  );
  return { root, source };
}

test("prepares browser assets and preserves isolation headers while blocking Preview indexing", async () => {
  const { root, source } = await fixture();
  await mkdir(path.join(source, ".well-known"));
  await mkdir(path.join(source, ".prerender"));
  await writeFile(
    path.join(source, ".well-known/security.txt"),
    "Contact: mailto:example@example.com",
  );
  await writeFile(path.join(source, ".prerender/server.js"), "server");
  await writeFile(path.join(source, "compiler.wasm"), Buffer.from([0, 97, 115, 109]));
  await writeFile(path.join(source, "worker.js"), "postMessage('ready')");
  await writeFile(path.join(source, "worker.js.map"), "map");
  await writeFile(path.join(source, "stillwind-manifest.json"), "{}");

  const output = await prepareSite(root, "play", "preview");
  expect((await readdir(output)).sort()).toEqual([
    ".well-known",
    "_headers",
    "compiler.wasm",
    "index.html",
    "robots.txt",
    "worker.js",
  ]);
  expect(await readFile(path.join(output, "compiler.wasm"))).toEqual(
    Buffer.from([0, 97, 115, 109]),
  );
  const headers = await readFile(path.join(output, "_headers"), "utf8");
  expect(headers).toContain("Cross-Origin-Opener-Policy: same-origin");
  expect(headers).toContain("Cross-Origin-Embedder-Policy: require-corp");
  expect(headers).toContain("X-Robots-Tag: noindex");
  expect(await readFile(path.join(output, "robots.txt"), "utf8")).toBe(
    "User-agent: *\nDisallow: /\n",
  );
});

test("preparing production replaces stale Preview files and preserves the source robots rules", async () => {
  const { root, source } = await fixture();
  const output = await prepareSite(root, "play", "preview");
  await writeFile(path.join(output, "stale.js"), "stale");
  await writeFile(path.join(source, "robots.txt"), "User-agent: *\nAllow: /\n");
  await prepareSite(root, "play", "production");
  expect(await readdir(output)).not.toContain("stale.js");
  expect(await readFile(path.join(output, "_headers"), "utf8")).toBe(
    await readFile(path.join(source, "_headers"), "utf8"),
  );
  expect(await readFile(path.join(output, "robots.txt"), "utf8")).toBe("User-agent: *\nAllow: /\n");
});

test("rejects an oversized asset before replacing the last prepared site", async () => {
  const { root, source } = await fixture();
  const output = await prepareSite(root, "play", "production");
  const file = await open(path.join(source, "compiler.wasm"), "w");
  try {
    await file.truncate(25 * 1024 * 1024 + 1);
  } finally {
    await file.close();
  }
  await expect(prepareSite(root, "play", "production")).rejects.toThrow("exceeds 25 MiB");
  expect(await readFile(path.join(output, "index.html"), "utf8")).toContain("Playground");
});
