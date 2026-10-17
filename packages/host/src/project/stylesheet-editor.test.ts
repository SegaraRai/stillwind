import { NativeDesignSystemLoader } from "@stillwind/native";
import { expect, test } from "vite-plus/test";

import { StylesheetEditor } from "./stylesheet-editor.ts";

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

test("directive and slot completions resolve to documentation without generated CSS", () => {
  const loader = new NativeDesignSystemLoader();
  loader.provide(0, "theme.css", "");
  for (const [text, label, expected] of [
    ["@app", "@apply", "@apply utilities;"],
    ["@stillwind-slot util", "utilities", "generated utility rules"],
  ]) {
    const editor = new StylesheetEditor("local.css", text, [], loader.editorQuery());
    const item = editor
      .query("local.css", { kind: "completion", offset: text.length })!
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
