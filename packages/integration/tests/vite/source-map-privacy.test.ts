import path from "node:path";

import { decodeCssSourceMap } from "@stillwind/host";
import { expect, onTestFinished, test, vi } from "vite-plus/test";

import { renderCssWithSourceMap } from "../../src/vite/runtime-source-map.ts";
import {
  compileContext,
  createTestServer,
  normalizeFile,
  redStyleSource,
} from "./vite-server-fixture.ts";

test("a client component's CSS map hides its server frontmatter and upstream source", async () => {
  const { root, project, server, origin } = await createTestServer();
  const componentFile = normalizeFile(path.join(root, "Component.astro"));
  const component = `---
import { style } from 'stillwind/astro';
const secret = 'COMPONENT_SOURCE_SENTINEL';
---
<div {...style('red')}>Content</div>`;
  const upstreamMap = {
    version: 3 as const,
    names: [],
    sources: ["private-server.ts"],
    sourceRoot: "file:///private/",
    sourcesContent: ["UPSTREAM_SOURCE_SENTINEL"],
    mappings: "AAAA;AAAA;AAAA;AAAA;AAAA;AAAA",
  };

  await project.compile("client", component, componentFile, {
    ...compileContext(server, "client"),
    getCombinedSourcemap: () => upstreamMap,
  });
  const response = await fetch(`${origin}/__stillwind/css`);
  const publicCss = await response.text();

  expect(publicCss).toMatch(/color:\s*red/);
  expect(publicCss).not.toContain("COMPONENT_SOURCE_SENTINEL");
  expect(publicCss).not.toContain("UPSTREAM_SOURCE_SENTINEL");
  expect(publicCss).not.toContain("private-server.ts");
  expect(publicCss).not.toContain("sourcesContent");
  expect(publicCss).not.toContain("file:");
  const parts = JSON.parse(publicCss).parts;
  expect(parts.length).toBeGreaterThan(0);
  for (const part of parts) {
    expect(part.map.sources).toEqual([expect.stringMatching(/^stillwind-source:[a-f0-9]{64}$/)]);
  }
});

test("the HTTP CSS response hides SSR source text and absolute paths", async () => {
  const { root, origin } = await compileServerStyles();

  const response = await fetch(`${origin}/__stillwind/css?environment=ssr`);
  const publicCss = await response.text();

  expect(publicCss).toMatch(/color:\s*red/);
  expectNoPrivateSource(publicCss, root);
});

test("HMR messages hide SSR source text and absolute paths", async () => {
  const { root, hmrSend } = await compileServerStyles();

  expect(hmrSend).toHaveBeenCalled();
  expect(JSON.stringify(hmrSend.mock.calls)).toMatch(/color:\s*red/);
  expectNoPrivateSource(JSON.stringify(hmrSend.mock.calls), root);
});

test.each([
  { output: "runtime module", moduleId: "\0stillwind:runtime" },
  { output: "CSS snapshot module", moduleId: "\0stillwind:snapshot.css" },
])("the $output hides SSR source text and absolute paths", async ({ moduleId }) => {
  const { root, server } = await compileServerStyles();

  const loadedModule = await server.environments.ssr.pluginContainer.load(moduleId);

  expect(loadedModule).toBeTruthy();
  expectNoPrivateSource(JSON.stringify(loadedModule), root);
});

test("combining SSR CSS parts keeps maps without publishing private sources", async () => {
  const { root, origin } = await compileServerStyles();
  const snapshot = await (await fetch(`${origin}/__stillwind/css?environment=ssr`)).json();

  const css = renderCssWithSourceMap(snapshot.parts);
  const { code, map } = decodeCssSourceMap(css);

  expect(code).toMatch(/color:\s*red/);
  expect(map.sources).toContainEqual(expect.stringMatching(/^stillwind-source:[a-f0-9]{64}$/));
  expectNoPrivateSource(JSON.stringify(map), root);
});

async function compileServerStyles() {
  const fixture = await createTestServer();
  const { root, project, server } = fixture;
  const environment = server.environments.ssr;
  await environment.pluginContainer.buildStart({});
  const hmrSend = vi.spyOn(environment.hot, "send");
  onTestFinished(() => hmrSend.mockRestore());
  const context = compileContext(server, "ssr");
  const serverFile = normalizeFile(path.join(root, "server.ts"));
  const serverSource = redStyleSource + "const secret = 'SSR_SOURCE_SENTINEL';";
  await project.compile("ssr", serverSource, serverFile, context);
  await project.publish("ssr", context);
  return {
    ...fixture,
    hmrSend,
  };
}

function expectNoPrivateSource(output: string, projectRoot: string) {
  expect(output).not.toContain("SSR_SOURCE_SENTINEL");
  expect(output).not.toContain("sourcesContent");
  expect(output).not.toContain("file:");
  expect(output).not.toContain(normalizeFile(projectRoot));
}
