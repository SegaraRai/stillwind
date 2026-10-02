import { createHash, randomUUID } from "node:crypto";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { createServer } from "node:net";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

import { chromium, type Browser, type Page } from "playwright";
import { expect } from "vite-plus/test";

import { fixtureSupport, type Fixture } from "./fixture.ts";
import { casePolicy, type CasePolicy } from "./policy.ts";
import { startNode, waitForServer } from "./process.ts";
import { createWorkspace } from "./workspace.ts";

const cli = fileURLToPath(new URL("./cli.ts", import.meta.url));

/**
 * Owns one fixture profile's processes, disposable source tree, browser contexts and evidence.
 */
export class FixtureApplication {
  #root: string;
  #workspace?: Awaited<ReturnType<typeof createWorkspace>>;
  #browser?: Browser;
  #server?: ReturnType<typeof startNode>;
  #page?: Page;
  #port = 0;
  #caseLabel = "";
  #built = false;
  readonly #errors: string[] = [];
  readonly #logs: string[] = [];
  readonly #checks: {
    name: string;
    status: string;
    artifacts: string;
    policy?: ReturnType<typeof casePolicy>;
  }[] = [];
  readonly #versions: Record<string, string> = {};
  readonly #results: string;

  constructor(
    readonly fixture: Fixture,
    readonly root: string,
    readonly profile: string,
  ) {
    this.#root = root;
    this.#results = join(root, "results", `${profile}-${randomUUID()}`);
  }

  /**
   * Returns the disposable directory after initialize has created it.
   */
  get directory(): string {
    if (!this.#workspace) {
      throw new Error("Fixture application has not been initialized");
    }
    return this.#workspace.directory;
  }

  /**
   * Creates the source copy, artifact directory and browser before any phase starts.
   */
  async initialize(): Promise<void> {
    await mkdir(this.#results, { recursive: true });
    this.#workspace = await createWorkspace(this.#root);
    for (const name of this.fixture.versions) {
      this.#versions[name] = JSON.parse(
        await readFile(join(this.#root, "node_modules", name, "package.json"), "utf8"),
      ).version;
    }
    this.#browser = await chromium.launch({
      channel: "chrome",
      headless: true,
    });
    this.#port = await availablePort();
  }

  /**
   * Clears browser errors and selects unique artifact names for a fully qualified case name.
   */
  beginCase(name: string): void {
    this.#errors.length = 0;
    this.#caseLabel = `${name
      .split(" > ")
      .at(-1)!
      .replace(/[^a-z0-9-]/gi, "-")
      .slice(0, 60)}-${createHash("sha256").update(name).digest("hex").slice(0, 12)}`;
  }

  /**
   * Records the completed result after Vitest has run the case and its cleanup hooks.
   * `expectedFailure` identifies a `test.fails` case; `passed` must account for that expectation.
   * Uses the artifact label established by beginCase for this case.
   */
  recordCase(name: string, passed: boolean, expectedFailure = false, policy?: CasePolicy): void {
    this.#checks.push({
      name,
      status: passed ? (expectedFailure ? "expected-failure" : "passed") : "failed",
      artifacts: this.#caseLabel,
      policy: policy && casePolicy(policy),
    });
  }

  /**
   * Starts development or production serving and waits for a successful HTTP response.
   */
  async start(mode: "dev" | "serve"): Promise<void> {
    this.#server = startNode(
      [cli, this.fixture.driver, mode, this.directory, String(this.#port)],
      this.directory,
      this.#env,
      600_000,
    );
    await waitForServer(this.#url, this.#server);
  }

  /**
   * Builds a fresh source copy for cold checks and primes the cache if a warm phase runs alone.
   */
  async build(temperature: "cold" | "warm"): Promise<void> {
    if (temperature === "cold") {
      await this.#workspace?.dispose();
      this.#workspace = await createWorkspace(this.#root);
      this.#built = false;
    }
    if (temperature === "warm" && !this.#built) {
      await this.#buildOnce();
    }
    await this.#buildOnce();
    await this.start("serve");
  }

  async #buildOnce(): Promise<void> {
    const build = startNode(
      [cli, this.fixture.driver, "build", this.directory, String(this.#port)],
      this.directory,
      this.#env,
      240_000,
    );
    try {
      expect(await build.done, build.logs()).toBe(0);
      this.#built = true;
    } finally {
      this.#logs.push(build.logs());
      await build.stop();
    }
  }

  /**
   * Stops the active server and keeps its output for the profile's report.
   */
  async stop(): Promise<void> {
    const server = this.#server;
    this.#server = undefined;
    if (server) {
      await server.stop();
      this.#logs.push(server.logs());
    }
  }

  /**
   * Opens a traced browser context at the fixture's declared page; closeCase owns its cleanup.
   */
  async open(javascript = true): Promise<Page> {
    if (!this.#browser) {
      throw new Error("Fixture browser has not been initialized");
    }
    const context = await this.#browser.newContext({
      javaScriptEnabled: javascript,
    });
    await context.tracing.start({
      screenshots: true,
      snapshots: true,
      sources: true,
    });
    const page = await context.newPage();
    this.#page = page;
    page.setDefaultTimeout(30_000);
    page.on("pageerror", (error) => this.#errors.push(error.message));
    page.on("console", (message) => {
      if (message.type() === "error") {
        this.#errors.push(message.text());
      }
    });
    await this.navigateHome(page);
    return page;
  }

  /**
   * Navigates the supplied page to the fixture's initial route.
   */
  async navigateHome(page: Page): Promise<void> {
    await page.goto(new URL(this.fixture.page.path, this.#url).href);
  }

  /**
   * Saves HTML under the current case's artifact name.
   */
  async captureHtml(page: Page): Promise<void> {
    await writeFile(join(this.#results, `${this.#caseLabel}.html`), await page.content());
  }

  /**
   * Saves failure evidence and the trace, closes the case's context, and rejects browser errors.
   */
  async closeCase(failed: boolean): Promise<void> {
    const page = this.#page;
    this.#page = undefined;
    if (page && !page.isClosed()) {
      try {
        if (failed || this.#errors.length) {
          await page
            .screenshot({
              path: join(this.#results, `${this.#caseLabel}-failure.png`),
              fullPage: true,
            })
            .catch(() => {});
        }
        await page.context().tracing.stop({
          path: join(this.#results, `${this.#caseLabel}.zip`),
        });
      } finally {
        await page.context().close();
      }
    }
    expect(this.#errors, "Browser errors").toEqual([]);
  }

  /**
   * Stops the server and browser, writes evidence, and removes the disposable directory even on failure.
   */
  async dispose(): Promise<void> {
    try {
      try {
        await this.stop();
      } finally {
        await this.#browser?.close();
      }
      await writeFile(join(this.#results, "server.log"), this.#logs.join("\n"));
      await writeFile(
        join(this.#results, "report.json"),
        JSON.stringify(
          {
            fixture: this.fixture.id,
            profile: this.profile,
            support: fixtureSupport(this.fixture, this.profile),
            versions: this.#versions,
            checks: this.#checks,
          },
          null,
          2,
        ),
      );
    } finally {
      await this.#workspace?.dispose();
    }
  }

  get #url(): string {
    return `http://localhost:${this.#port}`;
  }

  get #env(): NodeJS.ProcessEnv {
    return {
      STILLWIND_E2E_PROFILE: this.profile,
    };
  }
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
  await new Promise<void>((resolve, reject) =>
    server.close((error) => (error ? reject(error) : resolve())),
  );
  return address.port;
}
