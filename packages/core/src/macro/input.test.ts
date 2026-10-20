import path from "node:path";

import ts from "typescript";
import { expect, test } from "vite-plus/test";

function checkCompletions(code: string, check: (completions: (text: string) => string[]) => void) {
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
    check((text) => {
      const start = code.lastIndexOf(JSON.stringify(text));
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
});

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
});
