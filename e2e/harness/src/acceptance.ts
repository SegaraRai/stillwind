import { randomUUID } from "node:crypto";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { createServer } from "node:net";
import { join } from "node:path";
import { setTimeout as delay } from "node:timers/promises";
import { fileURLToPath } from "node:url";

import { chromium, type Page, type Browser } from "playwright";
import { expect, test } from "vite-plus/test";

import { defineFixture, fixtureProfiles, type Fixture } from "./fixture.ts";
import { startNode, waitForServer } from "./process.ts";
import { verifyInitialSemantics, verifySemanticInteractions } from "./semantics.ts";
import { createWorkspace, editFile } from "./workspace.ts";

const cli = fileURLToPath(new URL("./cli.ts", import.meta.url));

/** Registers the same browser contracts for every declared fixture profile. */
export function registerFixture(input: Fixture, packageUrl: URL): void {
  const fixture = defineFixture(input);
  const root = fileURLToPath(packageUrl);
  for (const profile of fixtureProfiles(fixture)) {
    test(`${fixture.id}/${profile}`, async () => {
      let workspace = await createWorkspace(root);
      const results = join(root, "results", `${profile}-${randomUUID()}`);

      const versions: Record<string, string> = {};

      const checks: { name: string; status: string; reason?: string }[] = [];
      let browser: Browser | undefined;
      let server: ReturnType<typeof startNode> | undefined;
      let activePage: Page | undefined;
      let failure: unknown;
      const errors: string[] = [];
      const logs: string[] = [];
      const env = { STILLWIND_E2E_PROFILE: profile };
      const open = async (url: string, javascript = true) => {
        const context = await browser!.newContext({ javaScriptEnabled: javascript });
        await context.tracing.start({ screenshots: true, snapshots: true, sources: true });
        const page = await context.newPage();
        page.setDefaultTimeout(30_000);
        activePage = page;
        page.on("pageerror", (error) => errors.push(error.message));
        page.on("console", (message) => {
          if (message.type() === "error") {
            errors.push(message.text());
          }
        });
        await page.goto(new URL(fixture.page.path, url).href);
        await waitForCss(page, fixture.page.target, fixture.page.css);
        for (const probe of fixture.page.styles ?? []) {
          await waitForCss(page, probe.selector, probe.css);
        }
        await verifyInitialSemantics(page, fixture.semantics);
        return page;
      };
      const close = async (page: Page, label: string) => {
        await page.context().tracing.stop({ path: join(results, `${label}.zip`) });
        await page.context().close();
        activePage = undefined;
      };
      const verify = async (url: string, phase: string) => {
        for (const [name, capability] of [
          ["ssr", fixture.ssr],
          ["prerender", fixture.prerender],
        ] as const) {
          if (!capability) {
            continue;
          }
          if (capability === true) {
            const page = await open(url, false);
            for (const counter of fixture.page.counters) {
              expect(await page.locator(counter.selector).textContent()).toBe(counter.initial);
            }
            await writeFile(join(results, `${phase}-${name}.html`), await page.content());
            await close(page, `${phase}-${name}`);
            checks.push({ name: `${phase}/${name}`, status: "passed" });
          } else {
            checks.push({
              name: `${phase}/${name}`,
              status: "not-applicable",
              reason: capability.reason,
            });
          }
        }
        const page = await open(url);
        await page.locator(fixture.page.ready).waitFor();
        await clickCounters(page, fixture);
        await verifySemanticInteractions(page, fixture.semantics);
        for (const probe of fixture.semantics ?? []) {
          checks.push({ name: `${phase}/composition/${probe.id}`, status: "passed" });
          checks.push({ name: `${phase}/conditions/${probe.id}`, status: "passed" });
        }
        checks.push({ name: `${phase}/rendering`, status: "passed" });
        checks.push({
          name: `${phase}/hydration`,
          status: fixture.hydration === true ? "passed" : "not-applicable",
          ...(fixture.hydration === true ? {} : { reason: fixture.hydration.reason }),
        });
        if (fixture.navigation) {
          const navigation = fixture.navigation;
          const sentinel = await setSentinel(page);
          await page.locator(navigation.link).click();
          await waitForCss(page, navigation.target, navigation.css);
          expect(await getSentinel(page)).toBe(
            navigation.document === "preserve" ? sentinel : undefined,
          );
          await page.locator(navigation.back).click();
          await waitForCss(page, fixture.page.target, fixture.page.css);
          checks.push({ name: `${phase}/navigation`, status: "passed" });
        }
        await close(page, `${phase}-rendering`);
      };
      const stop = async () => {
        if (server) {
          logs.push(server.logs());
          await server.stop();
          server = undefined;
        }
      };
      try {
        await mkdir(results, { recursive: true });
        for (const name of fixture.versions) {
          versions[name] = JSON.parse(
            await readFile(join(root, "node_modules", name, "package.json"), "utf8"),
          ).version;
        }
        browser = await chromium.launch({ channel: "chrome", headless: true });
        const port = await availablePort();
        const url = `http://localhost:${port}`;
        server = startNode(
          [cli, fixture.driver, "dev", workspace.directory, String(port)],
          workspace.directory,
          env,
          600_000,
        );
        await waitForServer(url, server);
        await verify(url, "dev");
        for (const [name, edit] of Object.entries(fixture.edits)) {
          const page = await open(url);
          await page.locator(fixture.page.ready).waitFor();
          await clickCounters(page, fixture);
          const sentinel = await setSentinel(page);
          const restore = await editFile(workspace.directory, edit);
          try {
            await waitForCss(page, edit.target || fixture.page.target, edit.css);
            await delay(400);
            await expect
              .poll(() => getSentinel(page), { timeout: 30_000, message: `${name}: document` })
              .toBe(edit.document === "preserve" ? sentinel : undefined);
            if (edit.state !== "not-applicable") {
              for (const counter of fixture.page.counters) {
                await expect
                  .poll(() => page.locator(counter.selector).textContent(), {
                    timeout: 30_000,
                    message: `${name}: state`,
                  })
                  .toBe(edit.state === "preserve" ? counter.clicked : counter.initial);
              }
            }
            checks.push({ name: `dev/edit/${name}`, status: "passed" });
          } finally {
            await restore();
          }
          await waitForCss(page, fixture.page.target, fixture.page.css);
          await close(page, `dev-edit-${name}`);
          // A fresh request must observe the restored source before the next edit.
          const restored = await open(url);
          await close(restored, `dev-restored-${name}`);
        }
        await stop();
        if (fixture.production !== false && process.env.STILLWIND_E2E_DEV_ONLY !== "1") {
          await workspace.dispose();
          workspace = await createWorkspace(root);
          for (const temperature of ["cold", "warm"]) {
            const build = startNode(
              [cli, fixture.driver, "build", workspace.directory, String(port)],
              workspace.directory,
              env,
              240_000,
            );
            try {
              expect(await build.done, build.logs()).toBe(0);
            } finally {
              logs.push(build.logs());
              await build.stop();
            }
            server = startNode(
              [cli, fixture.driver, "serve", workspace.directory, String(port)],
              workspace.directory,
              env,
              600_000,
            );
            await waitForServer(url, server);
            await verify(url, temperature);
            await stop();
          }
        }
        expect(errors, "Browser errors").toEqual([]);
      } catch (error) {
        failure = error;
        if (activePage && !activePage.isClosed()) {
          await activePage
            .screenshot({ path: join(results, "failure.png"), fullPage: true })
            .catch(() => {});
          await activePage
            .context()
            .tracing.stop({ path: join(results, "failure.zip") })
            .catch(() => {});
        }
        throw error;
      } finally {
        await stop();
        await browser?.close();
        await writeFile(join(results, "server.log"), logs.join("\n"));
        await writeFile(
          join(results, "report.json"),
          JSON.stringify(
            {
              fixture: fixture.id,
              profile,
              versions,
              checks,
              errors,
              failure: failure instanceof Error ? failure.stack : failure,
              directory: workspace.directory,
            },
            null,
            2,
          ),
        );
        await workspace.dispose();
      }
    });
  }
}

async function clickCounters(page: Page, fixture: Fixture) {
  for (const counter of fixture.page.counters) {
    expect(await page.locator(counter.selector).textContent()).toBe(counter.initial);
    await page.locator(counter.selector).click();
    await expect.poll(() => page.locator(counter.selector).textContent()).toBe(counter.clicked);
  }
}

async function waitForCss(page: Page, selector: string, css: Record<string, string>) {
  await expect
    .poll(
      () =>
        page.locator(selector).evaluate((element, properties) => {
          const style = getComputedStyle(element);
          return Object.fromEntries(
            properties.map((property) => [property, style[property as keyof CSSStyleDeclaration]]),
          );
        }, Object.keys(css)),
      { timeout: 30_000 },
    )
    .toEqual(css);
}

async function setSentinel(page: Page) {
  const value = randomUUID();
  await page.evaluate(
    (value) => Object.defineProperty(window, "__stillwindDocument", { value, configurable: true }),
    value,
  );
  return value;
}

async function getSentinel(page: Page) {
  return page.evaluate(
    () => (window as Window & { __stillwindDocument?: string }).__stillwindDocument,
  );
}

async function availablePort(): Promise<number> {
  const server = createServer();
  await new Promise<void>((resolve, reject) => {
    server.once("error", reject);
    server.listen(0, "localhost", resolve);
  });
  const address = server.address();
  if (!address || typeof address === "string") {
    throw new Error("Missing TCP address");
  }
  const port = address.port;
  await new Promise<void>((resolve, reject) =>
    server.close((error) => (error ? reject(error) : resolve())),
  );
  return port;
}
