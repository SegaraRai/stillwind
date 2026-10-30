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

/**
 * Removes the parts that only make sense on the website and turns site links into
 * relative file links. Links to other site files, such as `llms.txt`, point to the
 * deployed website because the package does not contain those files.
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
  return body;
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
}
