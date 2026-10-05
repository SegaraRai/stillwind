import { mkdtemp, readFile, rm, symlink, writeFile } from "node:fs/promises";
import path from "node:path";

import { afterEach, describe, expect, onTestFinished, test, vi } from "vite-plus/test";

import {
  createTestServer,
  internalModuleId,
  normalizeFile,
  packageRoot,
  redStyleSource,
} from "./vite-server-fixture.ts";

vi.mock("node:fs/promises", async (importOriginal) => {
  const original = await importOriginal<typeof import("node:fs/promises")>();
  return {
    ...original,
    readFile: vi.fn(original.readFile),
  };
});

afterEach(() => {
  vi.mocked(readFile).mockClear();
});

describe("unregistered CSS requests", () => {
  test.each(["value", "projection", "fragment", "producer"])(
    "a forged %s reference cannot read or watch a file",
    async (kind) => {
      const { root, project, server, origin } = await createTestServer();
      const secretFile = path.join(root, "secret.ts");
      await writeFile(secretFile, redStyleSource);
      const watcherAdd = vi.spyOn(server.watcher, "add");
      onTestFinished(() => watcherAdd.mockRestore());
      vi.mocked(readFile).mockClear();
      const encodedPath = Buffer.from(secretFile).toString("base64url");
      const forgedId = `\0stillwind:${kind}:${encodedPath}`;
      const plugins = server.environments.client.pluginContainer;

      await expect(plugins.resolveId(forgedId)).rejects.toThrow("Unregistered");
      await expect(plugins.load(forgedId)).rejects.toThrow("Unregistered");
      const response = await fetch(`${origin}/@id/__x00__stillwind:${kind}:${encodedPath}`);

      expect(response.status).toBe(500);
      expect(filesRead()).not.toContain(normalizeFile(secretFile));
      expect(watcherAdd).not.toHaveBeenCalled();
      expect(project.inputs().size).toBe(0);
    },
  );

  test("a direct file URL with ?stillwind.css cannot read the file", async () => {
    const { root, server, origin } = await createTestServer();
    const secretFile = path.join(root, "secret.ts");
    await writeFile(secretFile, redStyleSource);
    const watcherAdd = vi.spyOn(server.watcher, "add");
    onTestFinished(() => watcherAdd.mockRestore());
    vi.mocked(readFile).mockClear();

    const response = await fetch(`${origin}/secret.ts?stillwind.css`);

    expect(response.status).toBe(500);
    expect(filesRead()).not.toContain(normalizeFile(secretFile));
    expect(watcherAdd).not.toHaveBeenCalled();
  });

  test.each([
    { location: "a comment", code: "// import './secret.ts?stillwind.css';" },
    { location: "a string", code: `export const text = "import './secret.ts?stillwind.css';";` },
  ])("an import written inside $location does not register server CSS", async ({ code }) => {
    const { root, project, server } = await createTestServer();
    const secretFile = path.join(root, "secret.ts");
    const importer = normalizeFile(path.join(root, "main.ts"));
    await writeFile(secretFile, redStyleSource);
    await writeFile(importer, code);
    const watcherAdd = vi.spyOn(server.watcher, "add");
    onTestFinished(() => watcherAdd.mockRestore());
    vi.mocked(readFile).mockClear();

    await server.transformRequest("/main.ts");

    expect(filesRead()).not.toContain(normalizeFile(secretFile));
    const watchedFiles = watcherAdd.mock.calls.flatMap(([files]) => files);
    expect(watchedFiles.map(normalizeFile)).not.toContain(normalizeFile(secretFile));
    expect(project.inputs().size).toBe(0);
    expect(
      project.moduleRegistry().projection("./secret.ts?stillwind.css", importer),
    ).toBeUndefined();
  });
});

describe("imports of server CSS", () => {
  test("removing the importer's own styles keeps its imported server CSS", async () => {
    const { root, project, server, origin } = await createTestServer();
    const importer = normalizeFile(path.join(root, "main.ts"));
    await writeFile(
      path.join(root, "server.ts"),
      redStyleSource + "const secret = 'SERVER_SOURCE_SENTINEL';",
    );
    await writeFile(importer, redStyleSource + "import './server.ts?stillwind.css';");
    await server.transformRequest("/main.ts");

    await writeFile(importer, "import './server.ts?stillwind.css';");
    const importerModule = server.environments.client.moduleGraph.getModuleById(importer);
    if (!importerModule) {
      expect.fail("Expected main.ts in the client module graph");
    }
    server.environments.client.moduleGraph.invalidateModule(importerModule);
    const transformed = await server.transformRequest("/main.ts");

    expect(transformed?.code).toContain("stillwind:projection:");
    const reference = project.moduleRegistry().projection("./server.ts?stillwind.css", importer);
    if (!reference) {
      expect.fail("Expected the server CSS import to issue a reference");
    }
    expect(
      await server.environments.client.pluginContainer.load(internalModuleId(reference)),
    ).toBeTruthy();
    const publicCss = await (await fetch(`${origin}/__stillwind/css`)).text();
    expect(publicCss).toMatch(/color:\s*red/);
    expect(publicCss).not.toContain("SERVER_SOURCE_SENTINEL");
    expect((await fetch(`${origin}/server.ts?stillwind.css`)).status).toBe(500);
  });

  test("deleting the server file makes its issued CSS reference unusable", async () => {
    const { root, project, server } = await createTestServer();
    const target = normalizeFile(path.join(root, "server.ts"));
    const importer = normalizeFile(path.join(root, "main.ts"));
    await writeFile(target, redStyleSource);
    await writeFile(importer, "import './server.ts?stillwind.css';");
    await server.transformRequest("/main.ts");
    const reference = project.moduleRegistry().projection("./server.ts?stillwind.css", importer);
    if (!reference) {
      expect.fail("Expected the server CSS import to issue a reference");
    }
    await server.environments.client.pluginContainer.watchChange(target, { event: "delete" });

    await expect(
      server.environments.client.pluginContainer.load(internalModuleId(reference)),
    ).rejects.toThrow("Unregistered");
  });

  test("an import cannot read a file blocked by fs.deny", async () => {
    const { root, server } = await createTestServer();
    const blockedFile = path.join(root, "blocked.ts");
    await writeFile(blockedFile, redStyleSource);
    await writeFile(path.join(root, "main.ts"), "import './blocked.ts?stillwind.css';");
    vi.mocked(readFile).mockClear();

    await expect(server.transformRequest("/main.ts")).rejects.toThrow("access denied");

    expect(filesRead()).not.toContain(normalizeFile(blockedFile));
  });

  test("an import cannot follow a directory link outside fs.allow", async () => {
    const { root, server } = await createTestServer();
    const outsideRoot = await mkdtemp(path.join(packageRoot, ".test-private-"));
    onTestFinished(() => rm(outsideRoot, { recursive: true, force: true }));
    const secretFile = path.join(outsideRoot, "secret.ts");
    const linkedFile = path.join(root, "linked/secret.ts");
    await writeFile(secretFile, redStyleSource);
    await symlink(outsideRoot, path.join(root, "linked"), "junction");
    await writeFile(path.join(root, "main.ts"), "import './linked/secret.ts?stillwind.css';");
    vi.mocked(readFile).mockClear();

    await expect(server.transformRequest("/main.ts")).rejects.toThrow("access denied");

    expect(filesRead()).not.toContain(normalizeFile(secretFile));
    expect(filesRead()).not.toContain(normalizeFile(linkedFile));
  });
});

function filesRead() {
  return vi
    .mocked(readFile)
    .mock.calls.map(([file]) => file)
    .filter((file): file is string => typeof file === "string")
    .map(normalizeFile);
}
