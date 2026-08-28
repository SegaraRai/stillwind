import { execFileSync } from "node:child_process"
import { mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync } from "node:fs"
import { tmpdir } from "node:os"
import { basename, join, resolve } from "node:path"

const root = resolve(import.meta.dirname, "..")
const fixtureRoot = join(root, "fixtures", "tailwind-v4")
const manifest = readFileSync(join(fixtureRoot, "oracle.toml"), "utf8")
const cliPackage = /^cli_package\s*=\s*"([^"]+)"/m.exec(manifest)?.[1]
if (!cliPackage) throw new Error("oracle.toml must define cli_package")
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

function compile(input, candidates) {
  const directory = mkdtempSync(join(tmpdir(), "stillwind-tailwind-oracle-"))
  const source = join(directory, "input.css")
  const output = join(directory, "output.css")
  try {
    const importMatch = /(@import\s+"tailwindcss"[^;]*)(;)/.exec(input)
    if (!importMatch || importMatch[1].includes("source(")) {
      throw new Error("fixture Input CSS must import tailwindcss without source(...) so the oracle can isolate candidate detection")
    }
    const isolatedInput = input.replace(importMatch[0], `${importMatch[1]} source(none)${importMatch[2]}`)
    writeFileSync(source, `${isolatedInput}\n@source inline("${candidates}");\n`)
    execFileSync("vp", ["dlx", "--package", cliPackage, "--", "tailwindcss", "-i", source, "-o", output], {
      cwd: root,
      stdio: "pipe",
    })
    return readFileSync(output, "utf8").replaceAll("\r\n", "\n").trimEnd()
  } finally {
    rmSync(directory, { recursive: true, force: true })
  }
}

let changed = false
let caseCount = 0
for (const file of files) {
  const path = join(fixtureRoot, "cases", file)
  const markdown = readFileSync(path, "utf8").replaceAll("\r\n", "\n")
  const parsedCases = cases(markdown)
  if (parsedCases.length === 0) throw new Error(`${file} does not contain a valid fixture case`)
  caseCount += parsedCases.length
  const replacements = parsedCases.map((fixture) => ({
    fixture,
    css: compile(fixture.input, fixture.candidates),
  }))
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
