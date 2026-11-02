import { expect, it } from "vite-plus/test";

import { highlightCss, highlightScript, type SyntaxToken } from "./highlight";

const visible = (tokens: SyntaxToken[]) =>
  tokens.filter((token) => token.text.trim()).map((token) => [token.kind, token.text]);

it("covers the source with tokens in order", () => {
  const source = 'style(["px-4", on && "shadow-sm"], { "hover:": "bg-action" });\n';
  const tokens = highlightScript(source);
  expect(tokens.map((token) => token.text).join("")).toBe(source);
  tokens.forEach((token) => {
    expect(source.slice(token.start, token.start + token.text.length)).toBe(token.text);
  });
});

it("separates utilities, variants, and Stillwind arguments that are not utilities", () => {
  expect(
    visible(
      highlightScript(
        'const box = style.ref();\nstyle({ "aria-pressed:": "min-[713px]:px-6", [box.at("width >= 13rem")]: "grid" })',
      ),
    ),
  ).toEqual([
    ["keyword", "const"],
    ["identifier", "box"],
    ["punctuation", "="],
    ["api", "style"],
    ["punctuation", "."],
    ["api", "ref"],
    ["punctuation", "("],
    ["punctuation", ")"],
    ["punctuation", ";"],
    ["api", "style"],
    ["punctuation", "("],
    ["punctuation", "{"],
    ["punctuation", '"'],
    ["variant", "aria-pressed"],
    ["punctuation", ":"],
    ["punctuation", '"'],
    ["punctuation", ":"],
    ["punctuation", '"'],
    ["variant", "min-[713px]"],
    ["punctuation", ":"],
    ["utility", "px-6"],
    ["punctuation", '"'],
    ["punctuation", ","],
    ["punctuation", "["],
    ["identifier", "box"],
    ["punctuation", "."],
    ["api", "at"],
    ["punctuation", "("],
    ["punctuation", '"'],
    ["string", "width >= 13rem"],
    ["punctuation", '"'],
    ["punctuation", ")"],
    ["punctuation", "]"],
    ["punctuation", ":"],
    ["punctuation", '"'],
    ["utility", "grid"],
    ["punctuation", '"'],
    ["punctuation", "}"],
    ["punctuation", ")"],
  ]);
});

it("treats arguments of a keyframes result as utilities and plain object values as strings", () => {
  const kinds = (source: string, text: string) =>
    highlightScript(source).find((token) => token.text === text)?.kind;
  const keyframes =
    'const enter = style.keyframes({ from: "opacity-0" });\nstyle(enter("ease-out"));';
  expect(kinds(keyframes, "from")).toBe("property");
  expect(kinds(keyframes, "opacity-0")).toBe("utility");
  expect(kinds(keyframes, "ease-out")).toBe("utility");
  expect(kinds('{ className: "sw_reh4t" }', "sw_reh4t")).toBe("string");
  expect(kinds('{ "no-arbitrary-colors": "deny" }', "no-arbitrary-colors")).toBe("property");
  expect(kinds('{ "no-arbitrary-colors": "deny" }', "deny")).toBe("string");
});

it("classifies selectors, properties, functions, and values in CSS", () => {
  const source = ".sw_reh4t {\n  padding: calc(var(--spacing) * 4);\n}\n";
  const tokens = highlightCss(source);
  expect(tokens.map((token) => token.text).join("")).toBe(source);
  expect(visible(tokens)).toEqual([
    ["string", ".sw_reh4t"],
    ["punctuation", "{"],
    ["property", "padding"],
    ["punctuation", ":"],
    ["keyword", "calc"],
    ["punctuation", "("],
    ["keyword", "var"],
    ["punctuation", "("],
    ["identifier", "--spacing"],
    ["punctuation", ")"],
    ["punctuation", "*"],
    ["identifier", "4"],
    ["punctuation", ")"],
    ["punctuation", ";"],
    ["punctuation", "}"],
  ]);
});
