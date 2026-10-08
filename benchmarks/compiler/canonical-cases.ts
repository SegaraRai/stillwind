import { resolveFixtureCss } from "@stillwind-internal/tools-fixture-css/stillwind-css";
import { NativeDesignSystemLoader, type NativePreparedSource } from "@stillwind/native";

/** Inputs shared by compiler benchmarks and canonical performance checks. */
export const canonicalCases = [
  { name: "named", candidate: "p-4", expected: null },
  { name: "spacing", candidate: "p-[16px]", expected: "p-4" },
  { name: "color-match", candidate: "bg-[#f00]", expected: "bg-brand" },
  { name: "color-miss", candidate: "bg-[#123456]", expected: null },
  { name: "property-miss", candidate: "[border-color:#123456]", expected: null },
  { name: "color-modifier-match", candidate: "bg-[#f00]/50", expected: "bg-brand/50" },
  { name: "color-modifier-miss", candidate: "bg-[#123456]/50", expected: null },
  { name: "variant-miss", candidate: "[&:hover]:p-4", expected: null },
] as const;

const fullCss = resolveFixtureCss(
  '@import "stillwind"; @theme { --spacing:.25rem; --color-brand:red; }',
);

/** Loads the standard registry and the theme used by `canonicalCases`. */
export function canonicalLoader(): NativeDesignSystemLoader {
  const loader = new NativeDesignSystemLoader();
  loader.provide(0, "theme.css", fullCss, "untitled:theme.css");
  return loader;
}

/** Loads `count` distinct named colors that cannot match the measured arbitrary color. */
export function canonicalPalette(count: number): NativeDesignSystemLoader {
  const loader = new NativeDesignSystemLoader();
  const css =
    "@theme { " +
    Array.from(
      { length: count },
      (_, i) =>
        `--color-color${String(i).padStart(4, "0")}:rgb(${i % 256} ${Math.floor(i / 256) % 256} ${Math.floor(i / 65536) % 256});`,
    ).join(" ") +
    " } @utility bg-* { background-color:--value(--color-*, [color]); } @stillwind-slot utilities;";
  loader.provide(0, "theme.css", css, "untitled:theme.css");
  return loader;
}

/**
 * Creates `count` arbitrary colors, repeating one color when `unique` is false.
 * `independent` puts each color in its own style call. Otherwise one call contains
 * all colors and produces a diagnostic for every pair.
 */
export function canonicalSource(
  count: number,
  unique: boolean,
  independent = false,
): NativePreparedSource {
  const candidates = Array.from(
    { length: count },
    (_, i) => `bg-[#${(0x123456 + (unique ? i : 0)).toString(16)}]`,
  );
  return {
    kind: "direct",
    moduleId: "entry.ts",
    document: {
      uri: "untitled:entry.ts",
      format: "script",
      code:
        "import {style} from 'stillwind/react'; " +
        (independent
          ? candidates
              .map((candidate, i) => `export const card${i}=style('${candidate}');`)
              .join("\n")
          : `export const card=style('${candidates.join(" ")}');`),
    },
    syntax: { language: "ts", moduleKind: "module" },
    typeFacts: [],
  };
}
