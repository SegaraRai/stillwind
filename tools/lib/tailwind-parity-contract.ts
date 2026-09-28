import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

import {
  foldCalcExpressions,
  NativeDesignSystemLoader,
  NativeProjectSession,
} from "@stillwind/native";
import { fork, generate, parse, walk, type CssNode } from "css-tree";
import { transform } from "lightningcss";

import {
  comparableFixtureClasses,
  compileFixtureSource,
  fixtureProjectSource,
} from "./native-fixture-input.ts";

// These are explicit harness contracts, not general CSS equivalences. See
// docs/tailwind-comparison-contract.md. Never discard arbitrary layers, property
// registrations, var fallbacks, declaration order, or keyframes to pass parity.
const layers = new Set(["properties", "theme", "base", "components", "utilities"]);
const layerOrder = "@layer properties,theme,base,components,utilities;";
// @theme mixes declarations and nested keyframes. css-tree's unknown-at-rule
// heuristic otherwise consumes the first keyframes as part of a qualified rule.
const themeSyntaxConfig = {
  generic: true,
  atrule: {
    theme: {
      parse: {
        block(this: { Block(declarations: boolean): CssNode }) {
          return this.Block(true);
        },
      },
    },
  },
};
const themeSyntax = fork(themeSyntaxConfig);

/**
 * Removes supported lowercase prefix(...) and source(none) import modifiers for
 * the shared fixture and returns its candidate-prefix mapper. Mixed prefixes or
 * candidates missing the selected prefix fail assertions. This is a fixture
 * grammar adapter, not a general CSS import parser.
 */
export function parityFixture(input: string): {
  css: string;
  candidate: (value: string) => string;
} {
  const prefixes = [...input.matchAll(/@import\s+[^;]+\bprefix\(([a-z]+)\)/g)].map(
    (match) => match[1],
  );
  assert.ok(new Set(prefixes).size <= 1, "conflicting fixture prefixes");
  const prefix = prefixes[0];
  // Candidates are supplied explicitly: source(none) does not change their meaning.
  // Strip only these known import modifiers, on both compiler inputs. Prefix
  // migration tests utility semantics, not Stillwind support for Tailwind syntax.
  return {
    css: input.replace(/(@import\s+[^;]+);/g, (rule) =>
      rule.replace(/\s+prefix\([a-z]+\)|\s+source\(none\)/g, ""),
    ),
    candidate: (value) => {
      if (!prefix || !value) {
        return value;
      }
      assert.ok(value.startsWith(`${prefix}:`), `missing ${prefix}: fixture prefix`);
      return value.slice(prefix.length + 1);
    },
  };
}

/** Returns serialized keyframe names defined inside @theme; malformed CSS may throw. */
export function themeKeyframes(input: string): Set<string> {
  const names = new Set<string>();
  walk(themeSyntax.parse(input), {
    visit: "Atrule",
    enter(node) {
      if (node.name !== "theme" || !node.block) {
        return;
      }
      walk(node.block, {
        visit: "Atrule",
        enter(frame) {
          if (frame.name === "keyframes" && frame.prelude) {
            names.add(generate(frame.prelude));
          }
        },
      });
    },
  });
  return names;
}

export const defaultThemeKeyframes = themeKeyframes(
  readFileSync(new URL("../../packages/core/css/theme.css", import.meta.url), "utf8"),
);

/**
 * Returns theme-owned keyframe names, including imported defaults, excluding names
 * also defined outside @theme. The result bounds the Tailwind placement adapter;
 * malformed CSS may throw during parsing.
 */
export function fixtureThemeKeyframes(input: string): Set<string> {
  const names = new Set([
    ...(/@import\s+["']tailwindcss(?:\/theme)?["']/.test(input) ? defaultThemeKeyframes : []),
    ...themeKeyframes(input),
  ]);
  let depth = 0;
  walk(themeSyntax.parse(input), {
    enter(node: CssNode) {
      if (node.type !== "Atrule") {
        return;
      }
      if (node.name === "theme") {
        depth++;
      }
      // An authored same-name definition has its own collision semantics. Do not
      // move it merely because the default theme also supplies that name.
      if (node.name === "keyframes" && !depth && node.prelude) {
        names.delete(generate(node.prelude));
      }
    },
    leave(node: CssNode) {
      if (node.type === "Atrule" && node.name === "theme") {
        depth--;
      }
    },
  });
  return names;
}

/**
 * Normalizes CSS under the documented fixture layer and serialization contracts.
 * Pass tailwindThemeFrames only for expected Tailwind output: matching top-level
 * keyframes are placed in the theme layer. Stillwind output must omit that argument
 * so an incorrect placement remains visible.
 * Returns comparison text, not a proof of arbitrary CSS equivalence. Parsing and
 * strict Lightning CSS minification errors propagate.
 */
export function comparisonCss(
  css: string,
  tailwindThemeFrames: ReadonlySet<string> = new Set(),
): string {
  const ast = parse(css);
  if (ast.type !== "StyleSheet") {
    throw new Error("expected stylesheet");
  }
  const relocated: CssNode[] = [];
  ast.children.forEach((node, item, list) => {
    // Tailwind hoists @theme keyframes. The Stillwind preset intentionally places
    // them at its theme slot. Only the upstream side receives this adapter; a
    // wrong Stillwind layer/condition/body must still fail the comparison.
    if (
      node.type === "Atrule" &&
      node.name === "keyframes" &&
      node.prelude &&
      tailwindThemeFrames.has(generate(node.prelude))
    ) {
      relocated.push(node);
      list.remove(item);
    }
  });
  if (relocated.length) {
    const theme = ast.children
      .toArray()
      .find(
        (node) =>
          node.type === "Atrule" &&
          node.name === "layer" &&
          node.prelude &&
          generate(node.prelude) === "theme" &&
          node.block,
      );
    if (theme?.type === "Atrule" && theme.block) {
      for (const node of relocated) {
        theme.block.children.appendData(node);
      }
    } else {
      const wrapper = parse(`@layer theme{${relocated.map((node) => generate(node)).join("")}}`);
      if (wrapper.type !== "StyleSheet") {
        throw new Error("expected wrapper");
      }
      ast.children.prependList(wrapper.children);
    }
  }
  const result = transform({
    filename: "parity.css",
    code: Buffer.from(foldCalcExpressions(layerOrder + generate(ast))),
    minify: true,
    errorRecovery: false,
  }).code.toString();
  const normalized = parse(result, { parseCustomProperty: true });
  if (normalized.type !== "StyleSheet") {
    throw new Error("expected stylesheet");
  }
  // css-tree retains a whitespace-only custom-property value as Raw. CSS trims
  // declaration-edge whitespace; an empty value must not become an absent value.
  walk(normalized, {
    visit: "Declaration",
    enter(node) {
      if (node.property.startsWith("--") && !generate(node.value).trim()) {
        node.value = { type: "Raw", value: "" };
      }
    },
  });
  normalized.children.forEach((node, item, list) => {
    // Our shared prefix has already established these orders. Their repeated
    // top-level statements carry no further effect within this harness context.
    if (
      node.type === "Atrule" &&
      node.name === "layer" &&
      !node.block &&
      node.prelude &&
      generate(node.prelude)
        .split(",")
        .every((name) => layers.has(name.trim()))
    ) {
      list.remove(item);
    }
  });
  return generate(normalized);
}

const sessions = new Map<string, { session: NativeProjectSession; uses: number }>();
/**
 * Compiles one fixture module against fully loaded CSS and returns its output and
 * stylesheet. Sessions are cached by CSS text and replace only src/Parity.tsx.
 * First use and periodic direct compilation assert cache parity. Unresolved loader
 * requests or parity failures throw; compiler diagnostics remain in the output.
 */
export function compileParity(source: string, css: string) {
  let entry = sessions.get(css);
  if (!entry) {
    const loader = new NativeDesignSystemLoader();
    assert.equal(loader.provide(0, "stillwind.css", css).kind, "complete");
    entry = { session: new NativeProjectSession(loader, false), uses: 0 };
    sessions.set(css, entry);
  }
  const module = entry.session.compile([fixtureProjectSource(source, "src/Parity.tsx")]).modules[0];
  const output = { ...module, css: entry.session.css() };
  if (entry.uses++ % 250 === 0) {
    const direct = compileFixtureSource(source, "src/Parity.tsx", css);
    assert.equal(output.css, direct.css);
    assert.deepEqual(
      comparableFixtureClasses(output.classes),
      comparableFixtureClasses(direct.classes),
    );
    assert.deepEqual(output.diagnostics, direct.diagnostics);
  }
  // Native resource placeholders belong to the host. This harness supplies one
  // shared virtual asset base; only native-declared placeholders may be resolved.
  // Real owner-relative bundler resolution remains covered by host integration tests.
  for (const resource of output.resources) {
    output.css = output.css.replaceAll(resource.placeholder, resource.specifier);
  }
  return output;
}
