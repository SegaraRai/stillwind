import { readFile } from "node:fs/promises";

import { expect, test } from "vite-plus/test";

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
    contract: 'const primaryButton = style(button, "bg-accent text-white");',
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
  expect(html).toContain('<aside role="note" aria-label="Analysis scope" data-aside-type="note"');
  for (const path of ["docs/index.md", "llms-full.txt"]) {
    const markdown = await readFile(new URL(path, output), "utf8");
    expect(markdown).toContain("> **Analysis scope**");
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

test("includes all three guides and their code examples in the full documentation", async () => {
  const full = await readFile(new URL("llms-full.txt", output), "utf8");
  for (const { title, contract } of pages) {
    expect(full).toContain(`# ${title}`);
    expect(full).toContain(contract);
  }
});
