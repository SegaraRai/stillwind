import { readFile, readdir } from "node:fs/promises";

import { lintRules } from "stillwind/config";
import { expect, test } from "vite-plus/test";

import type { UtilityCatalog } from "../src/lib/docs/utilities/search";

const output = new URL("../dist/", import.meta.url);

const pages = [
  {
    path: "docs/",
    source: "docs/index.mdx",
    title: "Getting started",
    contract: 'css: "./src/stillwind.css"',
  },
  {
    path: "docs/frameworks/",
    source: "docs/frameworks.md",
    title: "Framework integrations",
    contract: 'import { stillwind } from "@stillwind/integration/astro";',
  },
  {
    path: "docs/style/",
    source: "docs/style.md",
    title: "The basics of style()",
    contract: 'const dangerButton = style(button, "bg-red-600");',
  },
];

test.each(pages)(
  "publishes HTML and Markdown for /$path",
  async ({ path, source, title, contract }) => {
    const html = await readFile(new URL(`${path}index.html`, output), "utf8");
    const markdown = await readFile(new URL(`${path}index.md`, output), "utf8");
    expect(html).toContain(`<h1>${title}</h1>`);
    expect(html).toContain(`href="/${path}index.md"`);
    expect(html).toContain('aria-current="page"');
    expect(html).toContain(
      `https://github.com/SegaraRai/stillwind/edit/main/apps/website/src/content/docs/${source}`,
    );
    expect(markdown).toContain(`# ${title}`);
    expect(markdown).toContain(contract);
    expect(markdown).toContain("```");
  },
);

test("renders an Aside and preserves its title and body in agent Markdown", async () => {
  const html = await readFile(new URL("docs/index.html", output), "utf8");
  expect(html).toContain(
    '<aside role="note" aria-label="Builds ignore files" data-aside-type="note"',
  );
  for (const path of ["docs/index.md", "llms-full.txt"]) {
    const markdown = await readFile(new URL(path, output), "utf8");
    expect(markdown).toContain("> **Builds ignore files**");
    expect(markdown).toContain("language server (LSP)");
  }
});

test("links the site index to a documentation index with reachable Markdown pages", async () => {
  const index = await readFile(new URL("llms.txt", output), "utf8");
  const docsIndex = await readFile(new URL("docs/llms.txt", output), "utf8");
  expect(index).toContain("/docs/llms.txt");
  for (const { path } of pages) {
    expect(docsIndex).toContain(`/${path}index.md`);
  }
  const links = Array.from(
    docsIndex.matchAll(/\]\((https?:\/\/[^)]+)\)/g),
    (match) => new URL(match[1]!),
  );
  expect(links.length).toBeGreaterThanOrEqual(pages.length);
  for (const link of links) {
    expect(await readFile(new URL(link.pathname.slice(1), output), "utf8")).toContain("# ");
  }
});

test("includes the entry guides and their code examples in the full documentation", async () => {
  const full = await readFile(new URL("llms-full.txt", output), "utf8");
  for (const { title, contract } of pages) {
    expect(full).toContain(`# ${title}`);
    expect(full).toContain(contract);
  }
});

test("publishes every authored doc through HTML, Markdown, navigation, and agent indexes", async () => {
  const source = new URL("../src/content/docs/", import.meta.url);
  const [files, full] = await Promise.all([
    readdir(source, { recursive: true }),
    readFile(new URL("llms-full.txt", output), "utf8"),
  ]);
  const indexes = new Set(["docs/llms.txt"]);
  const indexedPages = new Set<string>();
  for (const path of indexes) {
    const index = await readFile(new URL(path, output), "utf8");
    for (const [, target] of index.matchAll(/\]\((https?:\/\/[^)]+)\)/g)) {
      const pathname = new URL(target).pathname;
      if (pathname.endsWith("/llms.txt")) {
        indexes.add(pathname.slice(1));
      } else if (pathname.endsWith("/index.md")) {
        indexedPages.add(pathname);
      }
    }
  }
  for (const file of files.filter((name) => /\.mdx?$/.test(name))) {
    const text = await readFile(new URL(file.replaceAll("\\", "/"), source), "utf8");
    const slug = /^slug: (.+)$/m.exec(text)?.[1]?.replaceAll('"', "");
    const rawTitle = /^title: (.+)$/m.exec(text)![1]!;
    const title = rawTitle.startsWith('"') ? (JSON.parse(rawTitle) as string) : rawTitle;
    expect(slug, file).toBeTruthy();
    const path = `${slug}/`;
    const [html, markdown] = await Promise.all([
      readFile(new URL(`${path}index.html`, output), "utf8"),
      readFile(new URL(`${path}index.md`, output), "utf8"),
    ]);
    expect(html, file).toContain(`<h1>${title}</h1>`);
    expect(html, file).toContain(`href="/${path}index.md"`);
    expect(html, file).toContain(`href="/${path}" aria-current="page"`);
    expect(markdown, file).toContain(`# ${title}`);
    expect(indexedPages.has(`/${path}index.md`), file).toBe(true);
    expect(full, file).toContain(`# ${title}`);
    expect(html, file).toContain(
      `/edit/main/apps/website/src/content/docs/${file.replaceAll("\\", "/")}`,
    );
  }
});

test("covers all public lint IDs and every explicit compiler reporting code without changing case", async () => {
  const lintFiles = await readdir(new URL("../src/content/docs/docs/lints/", import.meta.url));
  const errorFiles = await readdir(new URL("../src/content/docs/docs/errors/", import.meta.url));
  expect(
    lintFiles
      .filter((file) => file !== "index.mdx")
      .map((file) => file.replace(/\.mdx$/, ""))
      .toSorted(),
  ).toEqual(Object.keys(lintRules).toSorted());
  const ledger = await readFile(
    new URL("../../../crates/diagnostics/src/compilation/code.rs", import.meta.url),
    "utf8",
  );
  const codes = [...ledger.matchAll(/=>\s*"(SW\d+)"/g)].map((match) => match[1]!);
  expect(
    errorFiles
      .filter((file) => file !== "index.mdx")
      .map((file) => file.replace(/\.mdx$/, ""))
      .toSorted(),
  ).toEqual(codes.toSorted());
  for (const code of codes) {
    expect(await readFile(new URL(`docs/errors/${code}/index.md`, output), "utf8")).toContain(code);
  }
  for (const [, category, block] of ledger.matchAll(
    /^    (Configuration|Authoring|Host|Internal) \{([\s\S]*?)^    \}/gm,
  )) {
    for (const [, code] of block.matchAll(/=>\s*"(SW\d+)"/g)) {
      const page = await readFile(
        new URL(`../src/content/docs/docs/errors/${code}.mdx`, import.meta.url),
        "utf8",
      );
      expect(page, code).toContain(
        `<CompilerError code="${code}" category="${category.toLowerCase()}"`,
      );
    }
  }
});

test.each(["lints", "errors", "macros", "macros/ref"])(
  "links every %s page from its index",
  async (directory) => {
    const source = new URL(`../src/content/docs/docs/${directory}/`, import.meta.url);
    const index = await readFile(new URL("index.mdx", source), "utf8");
    for (const file of await readdir(source)) {
      if (file === "index.mdx") {
        continue;
      }
      expect(index, file).toContain(`](/docs/${directory}/${file.replace(/\.mdx$/, "")}/)`);
    }
  },
);

test("preserves shared reference components in HTML and agent Markdown", async () => {
  const lint = await readFile(new URL("docs/lints/conflicting-utilities/index.md", output), "utf8");
  expect(lint).toContain("| Preset | Severity |");
  expect(lint).toContain("allowShorthandOverrides");
  expect(lint).toContain('style("p-2 p-4")');
  const error = await readFile(new URL("docs/errors/SW31008/index.md", output), "utf8");
  expect(error).toContain("Category: **host**");
  expect(error).toContain("UTF-8 byte offset: 1");
  const macro = await readFile(new URL("docs/macros/ref/index.md", output), "utf8");
  expect(macro).toContain("### Compiled code");
  expect(macro).toContain("container-type: inline-size;");
});

test("uses one finite utility catalog for interactive search, per-page Markdown, and the full agent document", async () => {
  const catalog = JSON.parse(
    await readFile(new URL("docs/utilities/catalog.json", output), "utf8"),
  ) as UtilityCatalog;
  const [markdown, full, html] = await Promise.all([
    readFile(new URL("docs/utilities/index.md", output), "utf8"),
    readFile(new URL("llms-full.txt", output), "utf8"),
    readFile(new URL("docs/utilities/index.html", output), "utf8"),
  ]);
  expect(catalog.records.length).toBeGreaterThan(1000);
  expect(new Set(catalog.records.map(({ name }) => name)).size).toBe(catalog.records.length);
  for (const text of [markdown, full]) {
    expect(text).toContain(catalog.revision);
    expect(text.match(/^### `[^`]+`$/gm)).toHaveLength(catalog.records.length);
    for (const record of [
      catalog.records[0]!,
      catalog.records.at(-1)!,
      catalog.records.find(({ name }) => name === "p-4")!,
    ]) {
      expect(text).toContain(`### \`${record.name}\``);
      if (record.css) {
        expect(text).toContain(record.css.trim());
      }
    }
  }
  expect(html).toContain('role="search"');
  expect(html).toContain('data-catalog="/docs/utilities/catalog.json"');
  expect(html).not.toContain(catalog.records.at(-1)!.css);
});
