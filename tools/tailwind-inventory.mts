import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { createWriteStream, mkdirSync, readFileSync, readdirSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { finished } from "node:stream/promises";
import { setImmediate } from "node:timers/promises";
import { createGzip } from "node:zlib";

import {
  compileReact,
  NativeDesignSystemLoader,
  NativeProjectSession,
  type ProjectModuleOutput,
} from "@stillwind/native";
import { generate, parse, walk, type CssNode } from "css-tree";

import { parseMarkdownFixtureCases } from "./lib/markdown-fixtures.ts";
import { hasEquivalentDeclarationOrder } from "./lib/property-order-parity.ts";
import { stillwindFixtureCss } from "./lib/stillwind-fixture-css.ts";
import { compileTailwindCss } from "./lib/tailwind-compiler.ts";
import {
  comparisonCss,
  parityFixture,
  fixtureThemeKeyframes,
} from "./lib/tailwind-parity-contract.ts";

const root = resolve(import.meta.dirname, "..");
const fixtureRoot = join(root, "fixtures/tailwind-v4");
const outputRoot = resolve(
  root,
  process.argv[process.argv.indexOf("--output") + 1] && process.argv.includes("--output")
    ? process.argv[process.argv.indexOf("--output") + 1]
    : "target/tailwind-inventory",
);
mkdirSync(outputRoot, { recursive: true });
const evidence = createGzip();
const evidenceFile = createWriteStream(join(outputRoot, "mismatches.jsonl.gz"));
evidence.pipe(evidenceFile);
const started = performance.now();
const counts: Record<string, number> = {};
const rows: Record<string, unknown>[] = [];
const exclusions: Record<string, unknown>[] = [];
const sessions = new Map<string, { session: NativeProjectSession; css: string; uses: number }>();
let directChecks = 0;
let checked = 0;

// The inventory and CI gate share the documented semantic comparison contract.
const minify = comparisonCss;
function selector(candidate: string): string {
  const escaped = candidate.replaceAll(/[^a-zA-Z0-9_-]/g, (value) =>
    value.codePointAt(0)! >= 0x80 ? value : `\\${value}`,
  );
  return /^\d/.test(candidate) ? `\\3${candidate[0]} ${escaped.slice(1)}` : escaped;
}
function equivalent(a: string, b: string): boolean {
  return a === b || hasEquivalentDeclarationOrder(a, b);
}
function withoutEmptyTheme(css: string): string {
  const ast = parse(css);
  if (ast.type !== "StyleSheet") throw new Error("expected stylesheet");
  ast.children.forEach((node, item, list) => {
    if (
      node.type === "Atrule" &&
      node.name === "layer" &&
      !node.block &&
      node.prelude &&
      generate(node.prelude) === "theme"
    )
      list.remove(item);
  });
  return generate(ast);
}
function tokens(css: string): string {
  return generate(parse(css, { parseCustomProperty: true }));
}
function withoutMetadata(css: string): string {
  const ast = parse(css);
  if (ast.type !== "StyleSheet") throw new Error("expected stylesheet");
  ast.children.forEach((node, item, list) => {
    if (
      node.type === "Atrule" &&
      (node.name === "property" ||
        (node.name === "layer" && node.prelude && generate(node.prelude) === "properties"))
    )
      list.remove(item);
  });
  return generate(ast);
}
function withoutRegisteredFallbacks(css: string): string {
  const ast = parse(css, { parseCustomProperty: true });
  const initials = new Map<string, string>();
  walk(ast, {
    visit: "Atrule",
    enter(node) {
      if (node.name !== "property" || !node.prelude || !node.block) return;
      const property = generate(node.prelude);
      node.block.children.forEach((child) => {
        if (child.type === "Declaration" && child.property === "initial-value")
          initials.set(property, generate(child.value));
      });
    },
  });
  walk(ast, {
    visit: "Function",
    enter(node) {
      if (node.name !== "var") return;
      const args = node.children.toArray();
      if (args[0]?.type !== "Identifier" || args[1]?.type !== "Operator" || args[1].value !== ",")
        return;
      const initial = initials.get(args[0].name);
      const fallback = args
        .slice(2)
        .map((child) => generate(child))
        .join("");
      if (initial !== undefined && initial === fallback) node.children.fromArray([args[0]]);
    },
  });
  return generate(ast);
}
function shape(css: string): { properties: string[]; rules: string[] } {
  const properties: string[] = [],
    rules: string[] = [];
  walk(parse(css, { parseCustomProperty: true }), {
    enter(node: CssNode) {
      if (node.type === "Declaration") properties.push(node.property);
      if (node.type === "Rule") rules.push(generate(node.prelude));
      if (node.type === "Atrule")
        rules.push(`@${node.name} ${node.prelude ? generate(node.prelude) : ""}`);
    },
  });
  return { properties: properties.sort(), rules };
}
function classify(expected: string, received: string): string {
  const a = withoutEmptyTheme(expected),
    b = withoutEmptyTheme(received);
  if (equivalent(a, b)) return "empty-theme-layer-only";
  const ta = tokens(a),
    tb = tokens(b);
  if (equivalent(ta, tb)) return "value-token-format";
  const fa = withoutRegisteredFallbacks(a),
    fb = withoutRegisteredFallbacks(b);
  if (equivalent(fa, fb)) return "registered-initial-fallback";
  if (equivalent(withoutMetadata(ta), withoutMetadata(tb)))
    return "property-registration-or-support";
  const sa = shape(ta),
    sb = shape(tb);
  if (JSON.stringify(sa.rules) !== JSON.stringify(sb.rules)) return "selector-or-rule-structure";
  if (JSON.stringify(sa.properties) !== JSON.stringify(sb.properties))
    return "declaration-inventory";
  return "declaration-value-or-order";
}

for (const file of readdirSync(join(fixtureRoot, "cases"))
  .filter((file) => file.endsWith(".fixture.md"))
  .sort()) {
  const markdown = readFileSync(join(fixtureRoot, "cases", file), "utf8");
  for (const fixture of parseMarkdownFixtureCases(markdown)) {
    const candidates = fixture.candidates.split(/\s+/).filter(Boolean);
    if (/@(?:source|config|plugin|reference)\b/.test(fixture.input)) {
      exclusions.push({
        file,
        case: fixture.name,
        candidates: candidates.length,
        reason: "existing-gate-scope",
      });
      continue;
    }
    const adapted = parityFixture(fixture.input);
    const frames = fixtureThemeKeyframes(adapted.css);
    const inputHash = createHash("sha256").update(fixture.input).digest("hex");
    let loaded = sessions.get(inputHash);
    if (!loaded) {
      try {
        const css = stillwindFixtureCss(adapted.css);
        const loader = new NativeDesignSystemLoader();
        assert.equal(loader.provide(0, "stillwind.css", css).kind, "complete");
        loaded = { session: new NativeProjectSession(loader, false), css, uses: 0 };
        sessions.set(inputHash, loaded);
        writeFileSync(join(outputRoot, `${inputHash.slice(0, 12)}.input.css`), fixture.input);
      } catch (error) {
        writeFileSync(join(outputRoot, `${inputHash.slice(0, 12)}.input.css`), fixture.input);
        for (const candidate of candidates.length ? candidates : [""]) {
          checked++;
          const category = "input-adapter-exception";
          counts[category] = (counts[category] ?? 0) + 1;
          const row = {
            id: checked,
            file,
            case: fixture.name,
            candidate,
            input: inputHash.slice(0, 12),
            category,
          };
          rows.push(row);
          evidence.write(JSON.stringify({ ...row, diagnostics: [String(error)] }) + "\n");
        }
        continue;
      }
    }
    for (const candidate of candidates.length ? candidates : [""]) {
      checked++;
      const source = `import { style } from "stillwind/react"; const props = style(${JSON.stringify(adapted.candidate(candidate))});`;
      const tailwind = await compileTailwindCss({
        css: adapted.css,
        candidates: [adapted.candidate(candidate)],
        base: fixtureRoot,
        from: join(fixtureRoot, "parity-theme.css"),
      });
      const expected = comparisonCss(tailwind, frames);
      let category: string;
      let actual = "",
        received = "";
      let diagnostics: unknown[] = [];
      let resources: unknown[] = [];
      try {
        const native: ProjectModuleOutput = loaded.session.compile([
          { path: "src/Parity.tsx", source },
        ]).modules[0];
        actual = loaded.session.css();
        if (loaded.uses++ % 250 === 0) {
          const direct = compileReact(source, "src/Parity.tsx", loaded.css);
          assert.equal(actual, direct.css, `session differs: ${file} ${candidate}`);
          assert.deepEqual(native.classes, direct.classes);
          assert.deepEqual(native.diagnostics, direct.diagnostics);
          directChecks++;
        }
        diagnostics = native.diagnostics;
        resources = native.resources;
        let resolvedCss = actual;
        for (const resource of native.resources)
          resolvedCss = resolvedCss.replaceAll(resource.placeholder, resource.specifier);
        received = minify(
          native.classes.length
            ? resolvedCss.replaceAll(
                native.classes[0].class_name,
                selector(adapted.candidate(candidate)),
              )
            : resolvedCss,
        );
        if (native.diagnostics.length || !native.classes.length) {
          category =
            equivalent(expected, received) && (expected === "" || !native.diagnostics.length)
              ? "accepted-empty"
              : "unresolved";
        } else if (
          equivalent(expected, received) ||
          (expected === "@layer utilities;" && received === "")
        )
          category = "accepted";
        else category = classify(expected, received);
      } catch (error) {
        // A validation disagreement invalidates the run instead of becoming a compiler mismatch.
        if (error instanceof assert.AssertionError) throw error;
        category = "exception";
        diagnostics = [String(error)];
      }
      counts[category] = (counts[category] ?? 0) + 1;
      if (checked % 250 === 0) {
        console.log(
          JSON.stringify({
            checked,
            file,
            candidate,
            elapsedSeconds: Math.round((performance.now() - started) / 1000),
          }),
        );
        await setImmediate();
      }
      const row = {
        id: checked,
        file,
        case: fixture.name,
        candidate,
        input: inputHash.slice(0, 12),
        category,
      };
      rows.push(row);
      if (!category.startsWith("accepted"))
        evidence.write(
          JSON.stringify({
            ...row,
            expected,
            received,
            rawTailwind: tailwind,
            rawStillwind: actual,
            diagnostics,
            resources,
          }) + "\n",
        );
    }
  }
  console.log(
    JSON.stringify({
      file,
      checked,
      elapsedSeconds: Math.round((performance.now() - started) / 1000),
      counts,
    }),
  );
}
evidence.end();
await finished(evidenceFile);
writeFileSync(
  join(outputRoot, "index.json"),
  JSON.stringify(
    {
      generatedAt: new Date().toISOString(),
      checked,
      directChecks,
      inputs: sessions.size,
      exclusions,
      counts,
      rows,
    },
    null,
    2,
  ) + "\n",
);
console.log(
  JSON.stringify({
    checked,
    directChecks,
    inputs: sessions.size,
    exclusions,
    counts,
    elapsedSeconds: Math.round((performance.now() - started) / 1000),
  }),
);

if (Object.keys(counts).some((category) => !category.startsWith("accepted"))) process.exitCode = 1;
