import path from "node:path";

import ts from "typescript";
import { expect, test } from "vite-plus/test";

function checkCompletions(
  code: string,
  check: (completions: (text: string, after?: string) => string[]) => void,
) {
  const fileName = path.join(import.meta.dirname, "completion.fixture.ts").replaceAll("\\", "/");
  const options: ts.CompilerOptions = {
    strict: true,
    target: ts.ScriptTarget.ES2024,
    module: ts.ModuleKind.NodeNext,
    moduleResolution: ts.ModuleResolutionKind.NodeNext,
    skipLibCheck: true,
    types: [],
  };
  const service = ts.createLanguageService({
    ...ts.sys,
    useCaseSensitiveFileNames: () => ts.sys.useCaseSensitiveFileNames,
    getScriptFileNames: () => [fileName],
    getScriptVersion: () => "1",
    getScriptSnapshot(name) {
      const text = name === fileName ? code : ts.sys.readFile(name);
      return text === undefined ? undefined : ts.ScriptSnapshot.fromString(text);
    },
    getCurrentDirectory: () => import.meta.dirname,
    getCompilationSettings: () => options,
    getDefaultLibFileName: ts.getDefaultLibFilePath,
  });
  try {
    expect(service.getSemanticDiagnostics(fileName)).toEqual([]);
    check((text, after) => {
      const context = after === undefined ? undefined : code.indexOf(after);
      if (context !== undefined) {
        expect(context).toBeGreaterThanOrEqual(0);
      }
      const start =
        context === undefined
          ? code.lastIndexOf(JSON.stringify(text))
          : code.indexOf(JSON.stringify(text), context);
      expect(start).toBeGreaterThanOrEqual(0);
      const result = service.getCompletionsAtPosition(fileName, start + 2, {});
      return (result?.entries ?? [])
        .filter((entry) => entry.kind === ts.ScriptElementKind.string)
        .map((entry) => entry.name);
    });
  } finally {
    service.dispose();
  }
}

test("style inputs do not suggest their inferred literals and retain ref template inputs", () => {
  checkCompletions(
    `import { style } from "stillwind/react";
declare const active: boolean;
const ref = style.ref();
style("flex gap-2");
style({ "hover:": "underline", "focus:": ["text-red-500"] });
style(["px-2 py-1", { "active:": "opacity-50" }]);
style(active ? "block" : "hidden");
style({ class: ["external-class", active && "external-active"], "--tone": active ? "red" : "blue" });
style(
  \`\${ref()}:flex\`,
  { "hover:": \`\${ref.in(ref)}:grid\` },
  [\`\${ref.at("width >= 40rem")}:block\`],
);
`,
    (completions) => {
      for (const text of [
        "flex gap-2",
        "underline",
        "text-red-500",
        "px-2 py-1",
        "opacity-50",
        "block",
        "hidden",
        "external-class",
        "external-active",
        "red",
        "blue",
      ]) {
        expect(completions(text)).toEqual([]);
      }
    },
  );
}, 30_000);

test("literal CVA values omit self-completions while selections, defaults, and compounds retain choices", () => {
  checkCompletions(
    `import { style } from "stillwind/react";
declare const active: boolean;
const button = style.cva("rounded", {
  variants: {
    size: { sm: "text-sm", lg: ["text-lg", { "hover:": "font-bold" }] },
    disabled: { false: null, true: "opacity-50" },
    count: { 0: "p-0", 1: "p-1" },
    tone: { warm: { "--tone": active ? "red" : "pink" }, cool: null },
  },
  defaultVariants: { size: "sm", disabled: false, count: 0 },
  compoundVariants: [
    { size: "sm", disabled: true, class: { "hover:": "shadow-md" } },
    { size: ["lg"], count: [0, 1], className: "underline" },
  ],
});
button({ size: "lg", disabled: true, count: 1 });
type LimitedVariants = { size: { sm: "text-xs" | "text-base" } };
style.cva<LimitedVariants>(null, { variants: { size: { sm: "text-xs" } } });
`,
    (completions) => {
      for (const text of [
        "text-sm",
        "text-lg",
        "font-bold",
        "opacity-50",
        "p-0",
        "p-1",
        "shadow-md",
        "underline",
        "red",
        "pink",
      ]) {
        expect(completions(text), text).toEqual([]);
      }
      for (const [text, after] of [
        ["lg", "button("],
        ["sm", "defaultVariants:"],
        ["sm", "compoundVariants:"],
        ["lg", "compoundVariants:"],
      ] as const) {
        expect(completions(text, after).toSorted(), after).toEqual(["lg", "sm"]);
      }
      expect(completions("text-xs").toSorted()).toEqual(["text-base", "text-xs"]);
    },
  );
}, 30_000);

test("finite macro choices remain available as string completions", () => {
  checkCompletions(
    `import { style } from "stillwind/react";
const ref = style.ref();
style(style.anchorScope("all"), ref.asContainer("size"));
const button = style.cva("flex", {
  variants: { size: { sm: "text-sm", lg: "text-lg" } },
  defaultVariants: { size: "sm" },
});
button({ size: "lg" });
style<readonly [{ class: "external-a" | "external-b" }]>({ class: "external-a" });
style<readonly [{ "--theme": "theme-a" | "theme-b" }]>({ "--theme": "theme-a" });
`,
    (completions) => {
      expect(completions("all")).toEqual(expect.arrayContaining(["all", "none"]));
      expect(completions("size")).toEqual(expect.arrayContaining(["size", "inline-size"]));
      expect(completions("sm")).toEqual(expect.arrayContaining(["sm", "lg"]));
      expect(completions("lg")).toEqual(expect.arrayContaining(["sm", "lg"]));
      expect(completions("external-a")).toEqual(["external-a", "external-b"]);
      expect(completions("theme-a")).toEqual(["theme-a", "theme-b"]);
    },
  );
}, 30_000);
