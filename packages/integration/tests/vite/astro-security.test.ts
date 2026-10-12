import { fork } from "node:child_process";
import { mkdir, mkdtemp, rm, writeFile } from "node:fs/promises";
import { createServer } from "node:http";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { expect, onTestFinished, test } from "vite-plus/test";

test("Astro SSR ignores the request Host when it reads CSS", async () => {
  const fixture = await createAstroFixture();

  const response = await fixture.requestWithAttackerHost();
  const html = await response.text();

  expect(response.status, html + fixture.serverErrors()).toBe(200);
  expect(html).toContain("data-stillwind-ssr");
  expect(html).toMatch(/color:\s*red/);
  expect(html).not.toContain("ATTACKER_RESPONSE_SENTINEL");
  expect(html).not.toContain("ASTRO_SERVER_SOURCE_SENTINEL");
  expect(fixture.attackerRequests()).toBe(0);
});

test("editing root.css updates Astro SSR styles without contacting the request Host", async () => {
  const fixture = await createAstroFixture();
  const initialResponse = await fixture.requestWithAttackerHost();
  expect(await initialResponse.text()).toMatch(/color:\s*red/);

  await writeFile(fixture.cssFile, "@utility red{color:blue} @stillwind-slot utilities;");

  await expect
    .poll(async () => {
      const response = await fixture.requestWithAttackerHost();
      const html = await response.text();
      expect(response.status, html + fixture.serverErrors()).toBe(200);
      return html;
    })
    .toMatch(/color:\s*blue/);
  expect(fixture.attackerRequests()).toBe(0);
});

test("Astro's public SSR CSS omits server source text", async () => {
  const fixture = await createAstroFixture();
  await fixture.requestWithAttackerHost();

  const response = await fetch(`${fixture.origin}/__stillwind/css?environment=ssr`);
  const publicCss = await response.text();

  expect(response.status).toBe(200);
  expect(publicCss).toMatch(/color:\s*red/);
  expect(publicCss).not.toContain("ASTRO_SERVER_SOURCE_SENTINEL");
  expect(publicCss).not.toContain("sourcesContent");
});

test("Astro SSR and its CSS watches survive repeated server restarts", async () => {
  const fixture = await createAstroFixture();
  expect(await (await fixture.requestWithAttackerHost()).text()).toMatch(/color:\s*red/);
  for (const color of ["blue", "purple"]) {
    await fixture.command("restart", "restarted");
    const response = await fixture.requestWithAttackerHost();
    expect(response.status, (await response.clone().text()) + fixture.serverErrors()).toBe(200);
    await writeFile(fixture.cssFile, `@utility red{color:${color}} @stillwind-slot utilities;`);
    await expect
      .poll(async () => {
        const response = await fixture.requestWithAttackerHost();
        const html = await response.text();
        expect(response.status, html + fixture.serverErrors()).toBe(200);
        return html;
      })
      .toMatch(new RegExp(`color:\\s*${color}`));
  }
});

test("closing Astro's client environment keeps its SSR bridge registered", async () => {
  const fixture = await createAstroFixture();
  await fixture.requestWithAttackerHost();
  await fixture.command("close-client", "client-closed");
  const response = await fixture.requestWithAttackerHost();
  const html = await response.text();
  expect(response.status, html + fixture.serverErrors()).toBe(200);
  expect(html).toMatch(/color:\s*red/);
});

async function createAstroFixture() {
  const packageRoot = fileURLToPath(new URL("../../", import.meta.url));
  const root = await mkdtemp(path.join(packageRoot, ".test-astro-security-"));
  const cssFile = path.join(root, "root.css");
  await mkdir(path.join(root, "src/pages"), { recursive: true });
  await writeFile(cssFile, "@utility red{color:red} @stillwind-slot utilities;");
  await writeFile(
    path.join(root, "src/pages/index.astro"),
    `---
import { style } from 'stillwind/astro';
const secret = 'ASTRO_SERVER_SOURCE_SENTINEL';
---
<html>
  <head><title>Security fixture</title></head>
  <body><p {...style('red')}>Styled</p></body>
</html>`,
  );
  let attackerRequestCount = 0;
  const attackerServer = createServer((_request, response) => {
    attackerRequestCount++;
    response.end("ATTACKER_RESPONSE_SENTINEL");
  });
  await new Promise<void>((resolve) => attackerServer.listen(0, "127.0.0.1", resolve));
  const attackerAddress = attackerServer.address();
  if (!attackerAddress || typeof attackerAddress === "string") {
    expect.fail("Expected the attacker server to listen on a TCP port");
  }
  const environment = { ...process.env };
  // Astro changes its server behavior when VITEST is set.
  delete environment.VITEST;
  const astroServer = fork(
    fileURLToPath(new URL("./astro-development-server.ts", import.meta.url)),
    [root],
    {
      env: environment,
      execArgv: [],
      silent: true,
    },
  );
  let stderr = "";
  astroServer.stderr?.on("data", (chunk) => {
    stderr = (stderr + String(chunk)).slice(-16_000);
  });
  const serverClosed = new Promise<void>((resolve) => astroServer.once("close", () => resolve()));
  onTestFinished(async () => {
    if (astroServer.connected) {
      astroServer.send("stop");
    }
    await serverClosed;
    await new Promise<void>((resolve, reject) =>
      attackerServer.close((error) => (error ? reject(error) : resolve())),
    );
    await rm(root, { recursive: true, force: true });
  });
  const port = await new Promise<number>((resolve, reject) => {
    astroServer.once("message", (message) => resolve((message as { port: number }).port));
    astroServer.once("exit", () => reject(new Error(stderr || "Astro exited before startup")));
  });
  const origin = `http://127.0.0.1:${port}`;
  return {
    origin,
    cssFile,
    requestWithAttackerHost: () =>
      fetch(origin, { headers: { Host: `127.0.0.1:${attackerAddress.port}` } }),
    attackerRequests: () => attackerRequestCount,
    serverErrors: () => stderr,
    command: (message: string, kind: string) =>
      new Promise<void>((resolve, reject) => {
        astroServer.once("message", (message) => {
          try {
            expect(message).toMatchObject({ kind });
            resolve();
          } catch (error) {
            reject(error);
          }
        });
        astroServer.send(message);
      }),
  };
}
