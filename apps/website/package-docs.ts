/**
 * Copies the Markdown pages of the website build into `packages/core/docs/`, so that
 * coding agents can read the documentation of the installed `stillwind` version.
 *
 * Run `astro build` first. The script replaces every file in the destination. It
 * fails when a skill or `AGENTS_TEMPLATE.md` names a page that the build lacks.
 */

import { existsSync } from "node:fs";
import { mkdir, readdir, readFile, rm, writeFile } from "node:fs/promises";
import path from "node:path";

const pages = path.resolve(import.meta.dirname, "dist/docs");
const packageRoot = path.resolve(import.meta.dirname, "../../packages/core");
const destination = path.join(packageRoot, "docs");
const siteOrigin = "https://stillwind.dev";

/** Routes relative to `/docs`, without slashes at either end. The root page is `""`. */
async function routes() {
  const found: string[] = [];
  for (const file of await readdir(pages, { recursive: true })) {
    const posix = file.replaceAll("\\", "/");
    if (posix === "index.md") {
      found.push("");
    } else if (posix.endsWith("/index.md")) {
      found.push(posix.slice(0, -"/index.md".length));
    }
  }
  if (!found.length) {
    throw new Error(`${pages} has no Markdown pages. Run the website build first.`);
  }
  return found.sort();
}

/**
 * Returns the file for a route. A route with child routes becomes `index.md` in its
 * own directory, so that `macros/ref.md` and `macros/ref/` never exist together.
 */
function fileFor(route: string, all: ReadonlySet<string>) {
  if (route === "") {
    return "index.md";
  }
  const parent = [...all].some((other) => other.startsWith(`${route}/`));
  return parent ? `${route}/index.md` : `${route}.md`;
}

const packageManagerNote =
  "> Command examples use `vp` (Vite+); substitute your project's package manager (such as `pnpm`, `npm`, or `bun`).";
const contentsThresholdBytes = 8000;
const contentsMinimumHeadings = 5;

/**
 * Removes the parts that only make sense on the website and turns site links into
 * relative file links. Links to other site files, such as `llms.txt`, point to the
 * deployed website because the package does not contain those files.
 * For bundled Markdown pages, adds a package-manager substitution note when shell
 * examples use `vp`, and inserts a `## Contents` section at the top of long pages.
 * `file` identifies the output file, and `files` maps docs routes to output files.
 * Throws when the index notice is missing or a page link has no output file.
 */
export function transformPackageMarkdown(
  markdown: string,
  file: string,
  files: ReadonlyMap<string, string>,
) {
  const origin = /Fetch the complete documentation index at: (\S+)\/llms\.txt/.exec(markdown)?.[1];
  if (!origin) {
    throw new Error(`${file}: the documentation index notice is missing`);
  }
  const body = markdown
    .replaceAll("\r\n", "\n")
    .replace(/^image: .*\n/m, "")
    .replace(/^> Documentation Index\n(?:> .*\n)*\n/m, "")
    .replace(/\n+Source: \S+\s*$/, "\n")
    .replace(/\[([^\]]*)\]\((\/[^)\s]*)\)/g, (_, text: string, target: string) => {
      const [pathname, hash] = target.split("#") as [string, string?];
      const route = pathname === "/docs" ? "" : /^\/docs\/(.*?)\/?$/.exec(pathname)?.[1];
      const linked = route === undefined ? undefined : files.get(route);
      if (linked === undefined) {
        if (/\.\w+$/.test(pathname)) {
          return `[${text}](${new URL(target, siteOrigin).href})`;
        }
        throw new Error(`${file}: unknown link ${target}`);
      }
      const relative = path.posix.relative(path.posix.dirname(file), linked);
      return `[${text}](${relative}${hash === undefined ? "" : `#${hash}`})`;
    });
  if (origin !== siteOrigin && body.includes(origin)) {
    throw new Error(`${file}: ${origin} remains after the conversion`);
  }
  return decorateBundledMarkdown(body);
}

/**
 * Inserts a package-manager note (when fenced shell blocks invoke `vp`) after the page's
 * `# ` heading, and a `## Contents` list (for long pages) before the first `## ` heading.
 */
function decorateBundledMarkdown(markdown: string): string {
  const { headings, firstHeadingOffset, usesVp } = scanMarkdownStructure(markdown);
  let output = markdown;
  if (
    firstHeadingOffset !== null &&
    markdown.length >= contentsThresholdBytes &&
    headings.length >= contentsMinimumHeadings
  ) {
    const slugCounts = new Map<string, number>();
    const items = headings.map((heading) => {
      const base = headingSlug(heading);
      const seen = slugCounts.get(base) ?? 0;
      slugCounts.set(base, seen + 1);
      const slug = seen === 0 ? base : `${base}-${seen}`;
      return `- [${heading}](#${slug})`;
    });
    output = `${output.slice(0, firstHeadingOffset)}## Contents\n\n${items.join("\n")}\n\n${output.slice(firstHeadingOffset)}`;
  }
  if (usesVp) {
    const titleMatch = /^# .+\n/m.exec(output);
    if (titleMatch) {
      const insertAt = titleMatch.index + titleMatch[0].length;
      output = `${output.slice(0, insertAt)}\n${packageManagerNote}\n${output.slice(insertAt)}`;
    }
  }
  return output;
}

function scanMarkdownStructure(markdown: string) {
  const headings: string[] = [];
  let firstHeadingOffset: number | null = null;
  let usesVp = false;
  let fence: string | null = null;
  let offset = 0;
  for (const line of markdown.split("\n")) {
    const lineStart = offset;
    offset += line.length + 1;
    const fenceMatch = /^(`{3,}|~{3,})/.exec(line);
    if (fenceMatch) {
      const marker = fenceMatch[1]!;
      if (fence === null) {
        fence = marker[0]!.repeat(marker.length);
      } else if (line.startsWith(fence)) {
        fence = null;
      }
      continue;
    }
    if (fence !== null) {
      if (/^\s*vp\s+(?:i|install|add|exec|run)\b/.test(line)) {
        usesVp = true;
      }
      continue;
    }
    const headingMatch = /^## (.+)$/.exec(line);
    if (headingMatch) {
      if (firstHeadingOffset === null) {
        firstHeadingOffset = lineStart;
      }
      headings.push(headingMatch[1]!.trim());
    }
  }
  return {
    headings,
    firstHeadingOffset,
    usesVp,
  };
}

function headingSlug(heading: string): string {
  return heading
    .toLowerCase()
    .replace(/`([^`]+)`/g, "$1")
    .replace(/[^\p{L}\p{N}\s-]/gu, "")
    .trim()
    .replace(/\s+/g, "-")
    .replace(/-+/g, "-");
}

async function generate() {
  const all = await routes();
  const set = new Set(all);
  const files = new Map(all.map((route) => [route, fileFor(route, set)]));
  const output = new Map<string, string>();
  for (const route of all) {
    const file = files.get(route)!;
    const source = path.join(pages, route, "index.md");
    output.set(file, transformPackageMarkdown(await readFile(source, "utf8"), file, files));
  }
  return output;
}

/**
 * Returns the Markdown files that the agent files of the package name in code spans
 * and that do not exist, relative to the package root. Paths with placeholders, such
 * as `docs/lints/<rule>.md`, are skipped.
 */
async function missingReferences(generated: ReadonlyMap<string, string>) {
  const skills = await readdir(path.join(packageRoot, "skills"));
  const missing: string[] = [];
  for (const file of ["AGENTS_TEMPLATE.md", ...skills.map((skill) => `skills/${skill}/SKILL.md`)]) {
    const text = await readFile(path.join(packageRoot, file), "utf8");
    for (const [, reference] of text.matchAll(/`([^`<>\s]+\.md)`/g)) {
      const relative = reference!.startsWith("../")
        ? path.posix.join(path.posix.dirname(file), reference!)
        : reference!.replace(/^node_modules\/stillwind\//, "");
      const found = relative.startsWith("docs/")
        ? generated.has(relative.slice("docs/".length))
        : !relative.startsWith("skills/") || existsSync(path.join(packageRoot, relative));
      if (!found) {
        missing.push(`${file}: ${reference}`);
      }
    }
  }
  return missing;
}

/**
 * Builds a single-file Markdown index (`llms.txt`) of every bundled documentation page,
 * grouped by top-level directory, with relative file paths and frontmatter descriptions.
 *
 * @param generated - Map from output paths relative to `docs/` to transformed Markdown text.
 * @returns The `llms.txt` text for the package `docs/` directory.
 */
export function generatePackageIndex(generated: ReadonlyMap<string, string>) {
  const sections = new Map<string, { file: string; title: string; description: string }[]>();
  for (const [file, markdown] of generated) {
    const slash = file.indexOf("/");
    const section = slash === -1 ? "" : file.slice(0, slash);
    const { title, description } = readFrontmatter(markdown);
    const entries = sections.get(section) ?? [];
    entries.push({ file, title: title || file, description });
    sections.set(section, entries);
  }
  const lines = [
    "# Stillwind documentation",
    "",
    "Index of the Markdown pages bundled in `docs/`. Paths are relative to this file.",
  ];
  const order = [...sections.keys()].sort((a, b) =>
    a === "" ? -1 : b === "" ? 1 : a.localeCompare(b),
  );
  for (const section of order) {
    lines.push("", `## ${section === "" ? "Pages" : section}`, "");
    const entries = sections.get(section)!.sort((a, b) => {
      const aIndex = a.file === "index.md" || a.file.endsWith("/index.md");
      const bIndex = b.file === "index.md" || b.file.endsWith("/index.md");
      return aIndex !== bIndex ? (aIndex ? -1 : 1) : a.file.localeCompare(b.file);
    });
    for (const { file, title, description } of entries) {
      lines.push(description ? `- [${title}](${file}): ${description}` : `- [${title}](${file})`);
    }
  }
  return `${lines.join("\n")}\n`;
}

function readFrontmatter(markdown: string) {
  const block = /^---\n([\s\S]*?)\n---\n/.exec(markdown)?.[1] ?? "";
  const field = (name: string) => {
    const raw = new RegExp(`^${name}:\\s*(.+)$`, "m").exec(block)?.[1]?.trim() ?? "";
    if (raw.startsWith('"') && raw.endsWith('"')) {
      return raw.slice(1, -1).replace(/\\(.)/g, "$1");
    }
    if (raw.startsWith("'") && raw.endsWith("'")) {
      return raw.slice(1, -1).replace(/''/g, "'");
    }
    return raw;
  };
  return {
    title: field("title"),
    description: field("description"),
  };
}

if (import.meta.main) {
  const generated = await generate();
  const missing = await missingReferences(generated);
  if (missing.length) {
    throw new Error(`Agent files name missing pages:\n${missing.join("\n")}`);
  }
  await rm(destination, { recursive: true, force: true });
  for (const [file, content] of generated) {
    const absolute = path.join(destination, file);
    await mkdir(path.dirname(absolute), { recursive: true });
    await writeFile(absolute, content);
  }
  await writeFile(path.join(destination, "llms.txt"), generatePackageIndex(generated));
}
