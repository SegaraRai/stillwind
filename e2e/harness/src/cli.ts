import { spawn } from "node:child_process";
import { readFile, writeFile } from "node:fs/promises";
import { createServer } from "node:http";
import { createRequire } from "node:module";
import { dirname, join, resolve } from "node:path";
import { pathToFileURL } from "node:url";

import { runFresh } from "./fresh.ts";
import { createWorkspace, fixturePath } from "./workspace.ts";

const [driver, mode, rootArgument, portArgument] = process.argv.slice(2);
const root = resolve(rootArgument || process.cwd());
const port = Number(portArgument || process.env.STILLWIND_E2E_PORT || 5173);
const require = createRequire(join(root, "package.json"));

process.chdir(root);
process.env.PORT = String(port);
process.env.NITRO_PORT = String(port);
process.env.STILLWIND_E2E_PORT = String(port);
process.env.NODE_ENV = mode === "dev" ? "development" : "production";
process.env.NEXT_TELEMETRY_DISABLED = "1";
process.env.GATSBY_TELEMETRY_DISABLED = "1";
process.env.GATSBY_CPU_COUNT = "2";
if (driver === "gatsby" && process.platform === "win32") {
  process.env.PARCEL_WORKERS = "0";
}
process.env.STILLWIND_E2E_STANDALONE =
  process.env.STILLWIND_E2E_PROFILE === "standalone" ? "1" : "0";
process.env.STILLWIND_E2E_BUNDLER =
  process.env.STILLWIND_E2E_PROFILE === "webpack" ? "webpack" : "turbopack";

const load = (name: string) => import(pathToFileURL(require.resolve(name)).href);

async function packageDirectory(name: string) {
  let directory = dirname(require.resolve(name));
  while (dirname(directory) !== directory) {
    try {
      if (JSON.parse(await readFile(join(directory, "package.json"), "utf8")).name === name) {
        return directory;
      }
    } catch {
      /* An entrypoint can be below directories without package metadata. */
    }
    directory = dirname(directory);
  }
  throw new Error(`Cannot locate package metadata: ${name}`);
}

if (driver === "fresh") {
  const workspace = mode === "check" ? await createWorkspace(root) : undefined;
  try {
    process.exitCode = await runFresh(workspace?.directory ?? root, mode, port);
  } finally {
    await workspace?.dispose();
  }
} else if (["webpack", "rspack", "rsbuild"].includes(driver)) {
  if (driver === "rsbuild") {
    const { createRsbuild } = await load("@rsbuild/core");
    const { default: config } = await import(pathToFileURL(join(root, "rsbuild.config.mjs")).href);
    const instance = await createRsbuild({ cwd: root, rsbuildConfig: config });
    if (mode === "build") {
      const result = await instance.build();
      await result.close();
    } else if (mode === "dev") {
      await instance.startDevServer();
    } else {
      await instance.preview();
    }
  } else if (mode === "serve") {
    createServer(async (request, response) => {
      try {
        const pathname = decodeURIComponent(
          new URL(request.url || "/", "http://localhost").pathname,
        );
        const file = fixturePath(
          join(root, "dist"),
          pathname === "/" ? "index.html" : pathname.slice(1),
        );
        const body = await readFile(file);
        response.setHeader(
          "content-type",
          file.endsWith(".css")
            ? "text/css"
            : file.endsWith(".js")
              ? "text/javascript"
              : "text/html",
        );
        response.end(body);
      } catch {
        response.statusCode = 404;
        response.end();
      }
    }).listen(port, "localhost");
  } else {
    const { default: config } = await import(
      pathToFileURL(join(root, `${driver}.config.mjs`)).href
    );
    const library = await load(driver === "webpack" ? "webpack" : "@rspack/core");
    const compiler = (driver === "webpack" ? library.default : library.rspack)(config);
    if (mode === "dev") {
      const devServer = await load(
        driver === "webpack" ? "webpack-dev-server" : "@rspack/dev-server",
      );
      const Server = driver === "webpack" ? devServer.default : devServer.RspackDevServer;
      await new Server(
        {
          port,
          host: "localhost",
          hot: true,
          client: { overlay: { errors: true, warnings: false } },
          ...config.devServer,
        },
        compiler,
      ).start();
    } else {
      await new Promise<void>((resolve, reject) =>
        compiler.run(
          (
            error: Error | null,
            stats:
              | {
                  hasErrors(): boolean;
                  toString(): string;
                  toJson(options: object): unknown;
                }
              | undefined,
          ) => {
            const evidence =
              stats && !stats.hasErrors()
                ? writeFile(
                    join(root, "build-stats.json"),
                    JSON.stringify(
                      stats.toJson({
                        all: false,
                        modules: true,
                        cachedModules: true,
                        nestedModules: true,
                        orphanModules: true,
                        groupModulesByAttributes: false,
                        groupModulesByCacheStatus: false,
                        modulesSpace: 100_000,
                        nestedModulesSpace: 100_000,
                      }),
                    ),
                  )
                : Promise.resolve();
            void evidence.then(
              () =>
                compiler.close((closeError: Error | null) =>
                  error || closeError || !stats || stats.hasErrors()
                    ? reject(
                        error ||
                          closeError ||
                          new Error(stats?.toString() || "Missing build stats"),
                      )
                    : resolve(),
                ),
              reject,
            );
          },
        ),
      );
    }
  }
} else if (driver === "vite") {
  const metadata = JSON.parse(await readFile(join(root, "package.json"), "utf8"));
  const vitePackage = require("vite-plus/package.json");
  const vite = await import(
    pathToFileURL(
      metadata.devDependencies?.vite7
        ? join(dirname(require.resolve("vite7/package.json")), "dist/node/index.js")
        : join(dirname(require.resolve("vite-plus/package.json")), vitePackage.exports["."].import),
    ).href
  );
  if (mode === "build") {
    const sveltekit3 =
      metadata.dependencies?.["@sveltejs/kit"] &&
      Number(require("@sveltejs/kit/package.json").version.split(".")[0]) >= 3;
    if (
      metadata.dependencies?.["@tanstack/react-start"] ||
      metadata.dependencies?.["@tanstack/solid-start"] ||
      metadata.dependencies?.["@solidjs/start"] ||
      sveltekit3
    ) {
      await (await vite.createBuilder({ root })).buildApp();
    } else {
      await vite.build({ root });
      if (metadata.dependencies?.["@builder.io/qwik"]) {
        await vite.build({
          root,
          build: {
            ssr: "src/entry.preview.tsx",
            outDir: "server",
          },
        });
      }
    }
  } else if (mode === "prepare") {
    await (await vite.createServer({ root })).close();
  } else if (mode === "dev") {
    const server = await vite.createServer({
      root,
      mode: metadata.dependencies?.["@builder.io/qwik"] ? "ssr" : undefined,
      server: {
        port,
        strictPort: true,
        host: "localhost",
      },
    });
    await server.listen();
    server.printUrls();
  } else if (mode === "serve") {
    const server = await vite.preview({
      root,
      preview: {
        port,
        strictPort: true,
        host: "localhost",
      },
    });
    server.printUrls();
  } else {
    throw new Error(`Unknown mode: ${mode}`);
  }
} else if (driver === "nuxt" && mode === "serve") {
  await import(pathToFileURL(join(root, ".output/server/index.mjs")).href);
} else if (driver === "react-router") {
  const packageName = mode === "serve" ? "@react-router/serve" : "@react-router/dev";
  const bin = join(dirname(require.resolve(`${packageName}/package.json`)), "bin.cjs");
  if (mode === "dev") {
    process.env.NODE_OPTIONS = "--conditions=development";
  }
  process.argv = [
    process.execPath,
    bin,
    ...(mode === "serve" ? ["build/server/index.js"] : [mode]),
  ];
  await import(pathToFileURL(bin).href);
} else {
  const packageName = driver === "nuxt" ? "@nuxt/cli" : driver;
  const packageRoot = await packageDirectory(packageName);
  const metadata = JSON.parse(await readFile(join(packageRoot, "package.json"), "utf8"));
  const binName =
    typeof metadata.bin === "string" ? metadata.bin : (Object.values(metadata.bin)[0] as string);
  const bin = join(packageRoot, binName);
  const command =
    driver === "gatsby"
      ? mode === "dev"
        ? "develop"
        : mode
      : mode === "serve"
        ? driver === "next"
          ? "start"
          : "preview"
        : mode;
  const args =
    driver === "next"
      ? [command, `--${process.env.STILLWIND_E2E_BUNDLER || "turbopack"}`]
      : [command];
  if (driver === "next" && mode === "serve") {
    args.pop();
  }
  if (mode !== "build") {
    args.push(driver === "next" ? "-p" : "--port", String(port));
  }
  if (driver === "astro" && mode !== "build") {
    // Astro locks can outlive forcibly stopped servers and identify reused PIDs.
    // The harness owns each server in an isolated workspace.
    args.push("--host", "localhost", "--ignore-lock");
  }
  process.argv = [process.execPath, bin, ...args];
  if (driver === "gatsby") {
    const child = spawn(process.execPath, [bin, ...args], {
      cwd: root,
      stdio: "inherit",
      windowsHide: true,
    });
    process.exitCode =
      (await new Promise<number | null>((resolve, reject) => {
        child.once("error", reject);
        child.once("exit", resolve);
      })) ?? 1;
  } else {
    await import(pathToFileURL(bin).href);
  }
}
