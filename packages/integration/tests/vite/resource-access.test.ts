import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import path from "node:path";

import { build } from "vite";
import { afterEach, expect, onTestFinished, test, vi } from "vite-plus/test";

import { createVitePlugins } from "../../src/vite/plugin.ts";
import { createViteProject } from "../../src/vite/project.ts";
import {
  compileContext,
  createTestServer,
  normalizeFile,
  packageRoot,
  redStyleSource,
} from "./vite-server-fixture.ts";

vi.mock("node:fs/promises", async (importOriginal) => {
  const original = await importOriginal<typeof import("node:fs/promises")>();
  return { ...original, readFile: vi.fn(original.readFile) };
});

afterEach(() => vi.mocked(readFile).mockClear());

test.each([
  { emission: "single", cssMinify: false, compression: "uncompressed" },
  { emission: "single", cssMinify: true, compression: "compressed" },
  { emission: "split", cssMinify: false, compression: "uncompressed" },
  { emission: "split", cssMinify: true, compression: "compressed" },
] as const)(
  "$emission output with $compression CSS copies the image URL but does not read the /@fs text",
  async ({ emission, cssMinify }) => {
    const root = await mkdtemp(path.join(packageRoot, ".test-resources-"));
    onTestFinished(() => rm(root, { recursive: true, force: true }));
    const privateFile = path.join(root, "private.txt");
    const literalFileUrl = `/@fs/${normalizeFile(privateFile)}`;
    await writeFile(privateFile, "PRIVATE_ASSET_SENTINEL");
    await writeFile(
      path.join(root, "icon.svg"),
      '<svg xmlns="http://www.w3.org/2000/svg"><text>PUBLIC_ICON</text></svg>',
    );
    await writeFile(
      path.join(root, "root.css"),
      `
.literal { content: ${JSON.stringify(literalFileUrl)}; }
@utility red { color: red; background-image: url('./icon.svg'); }
@stillwind-slot utilities;
`,
    );
    await writeFile(
      path.join(root, "index.html"),
      '<script type="module" src="/main.ts"></script>',
    );
    await writeFile(path.join(root, "auto.ts"), redStyleSource);
    await writeFile(
      path.join(root, "main.ts"),
      `
import './auto';
import css from 'stillwind/module-css';
${redStyleSource}
globalThis.css = css;
`,
    );
    const project = createViteProject({
      configFile: false,
      css: "root.css",
      emission,
    });
    vi.mocked(readFile).mockClear();

    const result = await build({
      configFile: false,
      root,
      logLevel: "silent",
      plugins: [createVitePlugins(project)],
      build: {
        write: false,
        minify: false,
        cssMinify,
        sourcemap: true,
        assetsInlineLimit: 0,
      },
    });
    if (Array.isArray(result) || !("output" in result)) {
      expect.fail("Expected one completed build");
    }
    const assets = result.output.filter((item) => item.type === "asset");
    const css = assets
      .filter((asset) => asset.fileName.endsWith(".css"))
      .map((asset) => String(asset.source))
      .join("\n");
    const imageAssets = assets.filter((asset) => asset.fileName.endsWith(".svg"));
    const mapAssets = assets.filter((asset) => asset.fileName.endsWith(".css.map"));
    const filesRead = vi
      .mocked(readFile)
      .mock.calls.map(([file]) => file)
      .filter((file): file is string => typeof file === "string")
      .map(normalizeFile);

    expect(imageAssets).toHaveLength(1);
    expect(css).toMatch(/url\([^)]*icon-[\w-]+\.svg/);
    expect(css).toContain(literalFileUrl);
    expect(filesRead).not.toContain(normalizeFile(privateFile));
    expect(assets.map((asset) => asset.fileName)).not.toContainEqual(
      expect.stringMatching(/\.txt$/),
    );
    expect(JSON.stringify(result.output)).not.toContain("PRIVATE_ASSET_SENTINEL");
    expect(mapAssets.length).toBeGreaterThan(0);
    for (const mapAsset of mapAssets) {
      expect(JSON.parse(String(mapAsset.source))).not.toHaveProperty("sourcesContent");
    }
  },
);

test("development CSS links resources in directories with URL-reserved characters to files that Vite serves", async () => {
  const { project, root, server, origin } = await createTestServer(
    {},
    "@utility red { color: red; background-image: url('./@scope+icons@1.0.0/icon.svg'); } @stillwind-slot utilities;",
    {},
    {
      "@scope+icons@1.0.0/icon.svg":
        '<svg xmlns="http://www.w3.org/2000/svg"><text>SCOPED_ICON</text></svg>',
    },
  );
  const entry = path.join(root, "entry.tsx");
  await writeFile(entry, redStyleSource);
  const context = compileContext(server, "client");
  await project.compile("client", redStyleSource, entry, context);
  await project.publish("client", context);

  const css = await (await fetch(`${origin}/__stillwind/style.css?environment=client`)).text();
  const url = /url\("?([^")]*icon\.svg)"?\)/.exec(css)?.[1];
  if (url === undefined) {
    expect.fail(`Expected an icon URL in the development CSS:\n${css}`);
  }
  const response = await fetch(new URL(url, origin));

  expect(await response.text()).toContain("SCOPED_ICON");
});
