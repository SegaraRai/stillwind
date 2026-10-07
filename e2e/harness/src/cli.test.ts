import { mkdir, writeFile } from "node:fs/promises";
import { createServer } from "node:net";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

import { expect, test } from "vite-plus/test";

import { startNode, waitForServer } from "./process.ts";
import { createWorkspace } from "./workspace.ts";

test("Astro preview restarts when a stale lock identifies another live process", async () => {
  const root = fileURLToPath(new URL("../../frameworks/astro7-plain/", import.meta.url));
  const cli = fileURLToPath(new URL("./cli.ts", import.meta.url));
  const workspace = await createWorkspace(root);
  let other: ReturnType<typeof startNode> | undefined;
  let preview: ReturnType<typeof startNode> | undefined;
  try {
    await writeFile(join(workspace.directory, "astro.config.ts"), "export default {};");
    await mkdir(join(workspace.directory, "dist"));
    await mkdir(join(workspace.directory, ".astro"));
    await writeFile(join(workspace.directory, "dist/index.html"), "<p>preview probe</p>");

    // The extra path models a PID reused by an Astro command in another workspace.
    other = startNode(
      ["-e", "console.log('ready'); setInterval(() => {}, 1000)", "astro/bin/astro.mjs"],
      workspace.directory,
      {},
      600_000,
    );
    await expect
      .poll(() => other!.logs(), {
        timeout: 5_000,
      })
      .toContain("ready");
    const listener = createServer();
    await new Promise<void>((resolve) => listener.listen(0, "127.0.0.1", resolve));
    const port = (listener.address() as { port: number }).port;
    await new Promise<void>((resolve, reject) =>
      listener.close((error) => (error ? reject(error) : resolve())),
    );
    const url = `http://localhost:${port}`;

    for (let restart = 0; restart < 3; restart++) {
      await writeFile(
        join(workspace.directory, ".astro/preview.json"),
        JSON.stringify({
          pid: other.child.pid,
          port,
          url,
          background: false,
          startedAt: new Date(0).toISOString(),
        }),
      );
      preview = startNode(
        [cli, "astro", "serve", workspace.directory, String(port)],
        workspace.directory,
        {},
        600_000,
      );
      await waitForServer(url, preview, 180_000);
      const response = await fetch(url);
      expect(await response.text()).toContain("preview probe");
      expect(other.child.exitCode).toBeNull();
      expect(other.child.signalCode).toBeNull();
      await preview.stop();
      preview = undefined;
    }
  } finally {
    await preview?.stop();
    await other?.stop();
    await workspace.dispose();
  }
}, 600_000);
