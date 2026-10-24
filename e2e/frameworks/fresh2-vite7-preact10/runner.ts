import { createBuilder, createServer } from "vite";

const [mode, rawPort] = Deno.args;
const port = Number(rawPort);
const config = {
  configFile: "vite.fresh.config.ts",
  configLoader: "native" as const,
};
if (mode === "build") {
  await (await createBuilder(config)).buildApp();
} else if (mode === "serve") {
  const entry = "./_fresh/server.js";
  const { default: server } = await import(entry);
  Deno.serve(
    {
      port,
      hostname: "localhost",
    },
    server.fetch,
  );
} else {
  const server = await createServer({
    ...config,
    server: {
      port,
      host: "localhost",
      strictPort: true,
    },
  });
  await server.listen();
  server.printUrls();
}
