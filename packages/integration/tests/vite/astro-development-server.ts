import process from "node:process";

import { stillwind } from "@stillwind/integration/astro";
import { dev } from "astro";
import type { ViteDevServer } from "vite";

let vite: ViteDevServer;

const server = await dev({
  root: process.argv[2],
  configFile: false,
  logLevel: "error",
  server: {
    host: "127.0.0.1",
    port: 0,
  },
  integrations: [
    stillwind({
      css: "root.css",
    }),
  ],
  vite: {
    plugins: [
      {
        name: "capture-development-server",
        configureServer(server) {
          vite = server;
        },
      },
    ],
    css: {
      devSourcemap: true,
    },
  },
});

process.send?.({
  port: server.address.port,
});

process.on("message", async (message) => {
  if (message === "restart") {
    await vite.restart();
    process.send?.({ kind: "restarted" });
    return;
  }
  if (message === "close-client") {
    await vite.environments.client.pluginContainer.close();
    process.send?.({ kind: "client-closed" });
    return;
  }
  await server.stop();
  process.disconnect?.();
});
