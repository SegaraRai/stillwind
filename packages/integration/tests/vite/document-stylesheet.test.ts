import { writeFile } from "node:fs/promises";
import path from "node:path";

import type { HotPayload } from "vite";
import { expect, onTestFinished, test, vi } from "vite-plus/test";

import { compileContext, createTestServer, redStyleSource } from "./vite-server-fixture.ts";

test("CSS changes reload served document stylesheet links through Vite's client", async () => {
  const { project, root, server, origin } = await createTestServer();
  const entry = path.join(root, "entry.tsx");
  await writeFile(entry, redStyleSource);
  const send = vi.spyOn(server.environments.client.hot, "send");
  onTestFinished(() => send.mockRestore());
  const cssUpdates = () =>
    send.mock.calls.flatMap(([payload]: unknown[]) => {
      const message = payload as HotPayload;
      return typeof message === "object" && message.type === "update"
        ? [message.updates.map((update) => [update.type, update.path])]
        : [];
    });
  const linked = "/__stillwind/style.css?environment=ssr";
  const selected = `${linked}&module=entry.tsx`;
  expect((await fetch(`${origin}${linked}&t=1`)).status).toBe(200);
  expect((await fetch(`${origin}${selected}`)).status).toBe(200);
  const context = compileContext(server, "ssr");

  await project.compile("ssr", redStyleSource, entry, context);
  await project.publish("ssr", context);
  await project.publish("ssr", context);

  expect(cssUpdates()).toEqual([
    [
      ["css-update", selected],
      ["css-update", linked],
    ],
  ]);
});
