import { NativeDesignSystemLoader } from "@stillwind/native";
import { expect, test } from "vite-plus/test";

import { applyEdits } from "../editor/sort.ts";
import { compilerOptions } from "./options.ts";
import { StylesheetEditor } from "./stylesheet-editor.ts";

test("CSS hovers and completion previews use default value comments and respect overrides", () => {
  const loader = new NativeDesignSystemLoader({
    rootFontSize: 20,
  });
  const text = `@theme { --spacing:.25rem; --color-brand:red; }
@utility space { padding:calc(var(--spacing) * 4); color:var(--color-brand); }
.a { @apply space; }`;
  loader.provide(0, "theme.css", text);
  const offset = text.lastIndexOf("space") + 2;
  const editor = new StylesheetEditor("theme.css", text, [], loader.editorQuery());
  const hover = editor.query("theme.css", {
    kind: "hover",
    offset,
  })!.hover!.css;
  expect(hover).toContain("padding: calc(var(--spacing) * 4) /* 1rem = 20px */;");
  expect(hover).toContain("color: var(--color-brand) /* red = #ff0000 */;");
  const item = editor
    .query("theme.css", {
      kind: "completion",
      offset,
    })!
    .completions!.find((item) => item.label === "space")!;
  expect(
    editor.query("theme.css", {
      kind: "resolve",
      candidate: item.candidate,
      input: item.input,
      snapshot: item.snapshot,
    })!.hover!.css,
  ).toBe(hover);
  const partial = new StylesheetEditor("theme.css", text, [], loader.editorQuery(), {
    preview: { colorEquivalents: false },
  }).query("theme.css", {
    kind: "hover",
    offset,
  })!.hover!.css;
  expect(partial).toContain("/* 1rem = 20px */");
  expect(partial).not.toContain("#ff0000");
  const plain = new StylesheetEditor("theme.css", text, [], loader.editorQuery(), {
    preview: {
      themeValues: false,
      pixelEquivalents: false,
      colorEquivalents: false,
    },
  }).query("theme.css", {
    kind: "hover",
    offset,
  })!.hover!.css;
  expect(plain).not.toContain("/*");
});

test("CSS variant snippets and selected details retain the completion settings", () => {
  const loader = new NativeDesignSystemLoader();
  loader.provide(
    0,
    "theme.css",
    `@variant hover { &:hover { @slot; } }
@stillwind-variant nth-* (value: [nth]) { &:--value(":nth-child({value})") { @slot; } }
@utility block { display: block; }`,
  );
  for (const text of ["/* 🦀 */ .a { @apply nth-; }", "/* 🦀 */ .a { @variant nth- {} }"]) {
    const offset = text.indexOf("nth-") + 4;
    const editor = new StylesheetEditor("local.css", text, [], loader.editorQuery());
    const items = editor.query("local.css", {
      kind: "completion",
      offset,
      snippets: true,
    })!.completions!;
    const item = items.find((item) => item.label.includes("[]"))!;
    expect(item.snippet).toContain("nth-[${1}]");
    expect(item.variant).toBe(true);
    expect(
      editor
        .query("local.css", {
          kind: "completion",
          offset,
        })!
        .completions!.some((item) => item.snippet),
    ).toBe(false);
    const filled = applyEdits(text, [
      {
        ...item,
        newText: item.newText.replace("[]", "[2n]") + (text.includes("@apply") ? "block" : ""),
      },
    ]);
    const completed = new StylesheetEditor("local.css", filled, [], loader.editorQuery());
    expect(
      completed.query("local.css", {
        kind: "hover",
        offset: filled.indexOf("nth-"),
      })?.hover?.css,
    ).toContain(":nth-child(2n)");
  }
  const text = ".a { @apply hov; }";
  const editor = new StylesheetEditor("local.css", text, [], loader.editorQuery());
  const item = editor
    .query("local.css", {
      kind: "completion",
      offset: text.indexOf("hov") + 3,
    })!
    .completions!.find((item) => item.label === "hover:")!;
  expect(
    editor.query("local.css", {
      kind: "resolve",
      candidate: item.candidate,
      input: item.input,
      snapshot: item.snapshot,
    })?.hover?.detail,
  ).toBe("&:hover");
});

test("CSS directive metadata and discovery settings are pinned through completion resolution", () => {
  const loader = new NativeDesignSystemLoader();
  loader.provide(0, "theme.css", "");
  const text = "/* 日本語 🦀 */ @stillwind-";
  const editor = new StylesheetEditor("local.css", text, [], loader.editorQuery());
  const complete = (registryDirectives: boolean) =>
    editor.query("local.css", {
      kind: "completion",
      offset: text.length,
      registryDirectives,
    })!.completions!;
  expect(complete(false).some((item) => item.label === "@stillwind-utility")).toBe(false);
  const enabled = complete(true);
  const registry = enabled.find((item) => item.label === "@stillwind-utility")!;
  expect(registry).toMatchObject({
    detail: "Registry authoring · Stillwind syntax",
    start: text.indexOf("@"),
    end: text.length,
  });
  expect(registry.sortText).toEqual(expect.any(String));
  const item = enabled.find((item) => item.label === "@stillwind-slot")!;
  const request = {
    kind: "resolve" as const,
    candidate: item.candidate,
    input: item.input,
    snapshot: item.snapshot,
  };
  expect(
    editor.query("local.css", {
      ...request,
      registryDirectives: false,
    })!.hover,
  ).toBeUndefined();
  expect(
    editor.query("local.css", {
      ...request,
      registryDirectives: true,
    })!.hover?.documentation,
  ).toContain("Authoring");
});

test("CSS completions replace original UTF-16 ranges and resolve across equivalent snapshots", () => {
  const loader = new NativeDesignSystemLoader();
  loader.provide(
    0,
    "theme.css",
    "@theme { --color-brand:red; } @utility flex {display:flex;} @custom-variant hover (&:hover);",
  );
  const text = "/* 日本語 🦀 */ .a { @apply flZZ; }";
  const offset = text.indexOf("flZZ") + 2;
  const editor = new StylesheetEditor("local.css", text, [], loader.editorQuery());
  const result = editor.query("local.css", { kind: "completion", offset });
  const item = result?.completions?.find((item) => item.label === "flex");
  expect(item).toMatchObject({
    start: text.indexOf("flZZ"),
    end: text.indexOf("flZZ") + 4,
    newText: "flex",
  });
  const resolve = {
    kind: "resolve" as const,
    candidate: item!.candidate,
    input: item!.input,
    snapshot: item!.snapshot,
  };
  const equivalent = new StylesheetEditor("local.css", text, [], loader.editorQuery());
  expect(equivalent.query("local.css", resolve)?.hover?.css).toContain("display: flex");
  const changedSettings = new StylesheetEditor("local.css", text, [], loader.editorQuery(), {
    useCssIf: false,
  });
  expect(changedSettings.query("local.css", resolve)?.hover).toBeUndefined();
  const changed = new NativeDesignSystemLoader();
  changed.provide(0, "theme.css", "@utility flex {display:grid;}");
  expect(
    new StylesheetEditor("local.css", text, [], changed.editorQuery()).query("local.css", resolve)
      ?.hover,
  ).toBeUndefined();
});

test("CSS colors and picker edits use original UTF-16 ranges and preserve utility modifiers", () => {
  const loader = new NativeDesignSystemLoader();
  loader.provide(
    0,
    "theme.css",
    "@theme { --color-brand:red; } @stillwind-utility bg-* {background-color:--compat-color(--value(--color-*, [color]), --alpha(--modifier()));} @custom-variant hover (&:hover);",
  );
  const text = "/* 日本語 🦀 */ .a { @apply hover:!bg-brand/50; color:var(--color-brand, blue); }";
  const editor = new StylesheetEditor("local.css", text, [], loader.editorQuery());
  const colors = editor.query("local.css", { kind: "colors" })!.colors!;
  expect(colors.map((color) => text.slice(color.start, color.end))).toEqual([
    "hover:!bg-brand/50",
    "var(--color-brand, blue)",
  ]);
  expect(colors[0].alpha).toBeCloseTo(0.5);
  const picked = {
    red: 0,
    green: 1,
    blue: 0,
    alpha: 0.25,
  };
  const changed = editor.query("local.css", {
    kind: "colorPresentation",
    ...colors[0],
    color: picked,
  })!.edits!;
  const next = applyEdits(text, changed);
  expect(next).toContain("hover:!bg-[rgb(0_255_0_/_");
  expect(next).toContain(")]/50;");
  const actual = new StylesheetEditor("local.css", next, [], loader.editorQuery()).query(
    "local.css",
    {
      kind: "colors",
    },
  )!.colors![0];
  for (const component of ["red", "green", "blue", "alpha"] as const) {
    expect(Math.abs(actual[component] - picked[component])).toBeLessThanOrEqual(2 / 255);
  }
  expect(
    editor.query("local.css", {
      kind: "colorPresentation",
      ...colors[0],
      color: {
        ...picked,
        alpha: 1,
      },
    })!.edits,
  ).toBeUndefined();
  const reference = editor.query("local.css", {
    kind: "colorPresentation",
    ...colors[1],
    color: {
      red: 0,
      green: 1,
      blue: 0,
      alpha: 1,
    },
  })!.edits!;
  expect(applyEdits(text, reference)).toContain("color:#00ff00;");
  expect(
    editor.query("local.css", {
      kind: "colorPresentation",
      start: 0,
      end: 1,
      color: picked,
    })!.edits,
  ).toBeUndefined();
});

test.each([false, true])(
  "CSS explicit sorting is independent of lint with applyStatements=%s",
  (statements) => {
    const config = {
      css: "theme.css",
      lint: false as const,
      sort: {
        applyUtilities: false,
        applyStatements: statements,
      },
    };
    const loader = new NativeDesignSystemLoader(compilerOptions(config));
    loader.provide(0, "theme.css", "@utility a {display:block;} @utility b {padding:1px;}");
    const text = ".a { @apply b; @apply b a; }";
    const editor = new StylesheetEditor("local.css", text, [], loader.editorQuery(), config);
    const sorted = applyEdits(
      text,
      editor.query("local.css", {
        kind: "sort",
      })!.edits!,
    );
    expect(sorted).toBe(text);
    const independent = ".a { @apply b; @apply a; }";
    const current = new StylesheetEditor(
      "local.css",
      independent,
      [],
      loader.editorQuery(),
      config,
    );
    expect(
      applyEdits(
        independent,
        current.query("local.css", {
          kind: "sort",
        })!.edits!,
      ),
    ).toBe(statements ? ".a { @apply a; @apply b; }" : independent);
    expect(
      current.query("local.css", {
        kind: "sort",
        protectedRanges: [
          {
            start: 0,
            end: independent.length,
          },
        ],
      })!.edits,
    ).toEqual([]);
  },
);

test("directive and slot completions resolve to documentation without generated CSS", () => {
  const loader = new NativeDesignSystemLoader();
  loader.provide(0, "theme.css", "");
  for (const [text, label, expected] of [
    ["@app", "@apply", "@apply utilities;"],
    ["@stillwind-slot util", "utilities", "generated utility rules"],
  ]) {
    const editor = new StylesheetEditor("local.css", text, [], loader.editorQuery());
    const item = editor
      .query("local.css", {
        kind: "completion",
        offset: text.length,
      })!
      .completions!.find((item) => item.label === label)!;
    expect(
      editor.query("local.css", {
        kind: "resolve",
        candidate: item.candidate,
        input: item.input,
        snapshot: item.snapshot,
      })?.hover?.documentation,
    ).toContain(expected);
  }
});
