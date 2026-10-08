import { mkdtemp, rm, writeFile } from "node:fs/promises";
import path from "node:path";
import { pathToFileURL } from "node:url";

import { fingerprint } from "@stillwind/native";
import { expect, test } from "vite-plus/test";

import { openNodeWorkspaces } from "../dist/node.js";

test("Node workers answer editor queries from unsaved source and stylesheet buffers", async () => {
  const root = await mkdtemp(path.resolve(import.meta.dirname, "../../host/.test-node-editor-"));
  const entry = path.join(root, "entry.tsx");
  const uri = pathToFileURL(entry).href;
  const cssUri = pathToFileURL(path.join(root, "theme.css")).href;
  const source =
    'import {style} from "stillwind/react"; export const card=style("p-[16px] bg-brand");';
  const css = `@theme { --spacing:.25rem; --color-brand:red; }
@utility p-* { padding:--value(--spacing(), [length]); }
@utility bg-* { background-color:--value(--color-*, [color]); }
@variant hover { &:hover { @slot; } } @stillwind-slot utilities;`;
  let opened: Awaited<ReturnType<typeof openNodeWorkspaces>> | undefined;
  try {
    await writeFile(
      path.join(root, "stillwind.config.ts"),
      'export default {projects:[{css:"theme.css",files:["*.tsx"]}]}',
    );
    await writeFile(path.join(root, "theme.css"), css);
    await writeFile(entry, 'import {style} from "stillwind/react"; const card=style("bg-brand");');
    opened = await openNodeWorkspaces({ workspaceFolders: [pathToFileURL(root).href] });
    expect([...(opened.errors ?? [])]).toEqual([]);
    const workspace = [...opened.workspaces.values()][0];
    await workspace.update(uri, source, 1);
    const fixes = await workspace.query!(uri, { kind: "fixes" });
    expect(fixes[0].result).toMatchObject({
      revision: fingerprint(source),
      edits: [{ newText: "p-4" }],
    });
    const hover = await workspace.query!(uri, { kind: "hover", offset: source.indexOf("card") });
    expect(hover[0].result.hover?.css).toContain("&");
    expect(hover[0].result.hover?.css).toContain("padding");
    const completions = await workspace.query!(uri, {
      kind: "completion",
      offset: source.indexOf("bg-brand"),
    });
    expect(completions[0].result.completions?.some((item) => item.label === "hover:")).toBe(true);
    await workspace.update(cssUri, css.replace("--color-brand:red", "--color-brand:blue"), 1);
    const colors = await workspace.query!(uri, { kind: "colors" });
    expect(colors[0].result.colors).toMatchObject([{ red: 0, green: 0, blue: 1, alpha: 1 }]);
    expect(await workspace.query!(uri, { kind: "fixes" }, AbortSignal.abort())).toEqual([]);
    const updated = source.replace("p-[16px]", "p-4");
    await workspace.update(uri, updated, 2);
    expect((await workspace.query!(uri, { kind: "fixes" }))[0].result).toMatchObject({
      revision: fingerprint(updated),
      edits: [],
    });
  } finally {
    for (const workspace of opened?.workspaces.values() ?? []) {
      await workspace.close();
    }
    await rm(root, { recursive: true, force: true });
  }
});
