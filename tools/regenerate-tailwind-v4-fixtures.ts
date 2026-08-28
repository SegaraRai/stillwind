import { readFileSync, readdirSync, writeFileSync } from "node:fs"
import { basename, dirname, join, resolve } from "node:path"
import { pathToFileURL } from "node:url"
import { compile as compileTailwind } from "tailwindcss"

const root = resolve(import.meta.dirname, "..")
const fixtureRoot = join(root, "fixtures", "tailwind-v4")
const write = process.argv.includes("--write")
const files = readdirSync(join(fixtureRoot, "cases"))
  .filter((file) => file.endsWith(".fixture.md"))
  .sort()
if (files.length > 500) throw new Error("fixture file budget exceeded (maximum: 500)")

function cases(markdown) {
  const pattern = /## `([^`]+)`\r?\n\n### Input CSS\r?\n\r?\n```css\r?\n([\s\S]*?)\r?\n```\r?\n\r?\n### Candidates\r?\n\r?\n```text\r?\n([\s\S]*?)\r?\n```\r?\n\r?\n### Expected CSS\r?\n\r?\n```css\r?\n([\s\S]*?)\r?\n```/g
  return [...markdown.matchAll(pattern)].map((match) => ({
    name: match[1],
    input: match[2],
    candidates: match[3].trim().replace(/\s+/g, " "),
    expected: match[4],
    expectedStart: match.index + match[0].lastIndexOf("### Expected CSS\n\n```css\n") + "### Expected CSS\n\n```css\n".length,
    expectedEnd: match.index + match[0].length - "\n```".length,
  }))
}

function stylesheetPath(id, base) {
  const tailwind = join(root, "tools", "node_modules", "tailwindcss")
  const builtins = new Map([
    ["tailwindcss", "index.css"],
    ["tailwindcss/theme", "theme.css"],
    ["tailwindcss/preflight", "preflight.css"],
    ["tailwindcss/utilities", "utilities.css"],
  ])
  return builtins.has(id) ? join(tailwind, builtins.get(id)) : resolve(base, id)
}

async function compile(input, candidates) {
  const source = join(fixtureRoot, "fixture.css")
  const compiler = await compileTailwind(input, {
    base: fixtureRoot,
    from: source,
    async loadStylesheet(id, base) {
      const path = stylesheetPath(id, base)
      return { path, base: dirname(path), content: readFileSync(path, "utf8") }
    },
    async loadModule(id, base) {
      const path = resolve(base, id)
      const loaded = await import(pathToFileURL(path).href)
      return { path, base: dirname(path), module: loaded.default ?? loaded }
    },
  })
  return compiler.build(candidates ? candidates.split(" ") : []).replaceAll("\r\n", "\n").trimEnd()
}

let changed = false
let caseCount = 0
for (const file of files) {
  const path = join(fixtureRoot, "cases", file)
  const markdown = readFileSync(path, "utf8").replaceAll("\r\n", "\n")
  const parsedCases = cases(markdown)
  if (parsedCases.length === 0) throw new Error(`${file} does not contain a valid fixture case`)
  caseCount += parsedCases.length
  const replacements = []
  for (const fixture of parsedCases) {
    replacements.push({ fixture, css: await compile(fixture.input, fixture.candidates) })
  }
  let next = markdown
  for (const { fixture, css } of replacements.reverse()) {
    next = `${next.slice(0, fixture.expectedStart)}${css}${next.slice(fixture.expectedEnd)}`
  }
  if (next !== markdown) {
    changed = true
    if (write) writeFileSync(path, next)
    else console.error(`${basename(path)} is stale; run with --write`)
  }
}

if (changed && !write) {
  process.exitCode = 1
} else {
  console.log(`${write ? "Regenerated" : "Verified"} ${caseCount} Tailwind v4 fixture cases in ${files.length} files.`)
}
