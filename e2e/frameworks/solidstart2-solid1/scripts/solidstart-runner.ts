import { createBuilder, createServer } from "vite-plus";

const [mode, root] = process.argv.slice(2);
process.chdir(root);
if (mode === "build") {
  const builder = await createBuilder({ root });
  await builder.buildApp();
} else {
  const server = await createServer({ root });
  await server.listen();
  server.printUrls();
}
