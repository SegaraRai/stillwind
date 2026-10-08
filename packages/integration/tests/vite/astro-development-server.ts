import { stillwind } from "@stillwind/integration/astro";
import { dev } from "astro";

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
    css: {
      devSourcemap: true,
    },
  },
});

process.send?.({
  port: server.address.port,
});

process.once("message", async () => {
  await server.stop();
  process.disconnect?.();
});
