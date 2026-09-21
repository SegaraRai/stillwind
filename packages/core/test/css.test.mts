import assert from "node:assert/strict";
import test from "node:test";

import { cssText, stylesheetLinks, styleSheets } from "../dist/css.js";

void test("manifest selections preserve occurrence positions across reverse requests", () => {
  const manifest = {
    version: 1 as const,
    revision: "build",
    parts: [
      { id: "red:first", href: "/red.css" },
      { id: "blue", href: "/blue.css" },
      { id: "red:last", href: "/red.css" },
    ],
    modules: { a: ["red:first", "red:last"], b: ["blue"] },
  };
  assert.deepEqual(
    stylesheetLinks(manifest, ["b", "a"]),
    manifest.parts.map((part, order) => ({ ...part, order })),
  );
  assert.throws(() => stylesheetLinks(manifest, ["missing"]), /Unknown Stillwind/);
  assert.equal(
    cssText([
      { key: "red", css: "red" },
      { key: "blue", css: "blue" },
      { key: "red", css: "red" },
    ]),
    "redbluered",
  );
});

void test("constructed sheets share content within a Document and retain repeated positions", () => {
  class Sheet {
    css = "";
    replaceSync(css: string) {
      this.css = css;
    }
  }
  const makeDocument = () => ({ defaultView: { CSSStyleSheet: Sheet } }) as unknown as Document;
  const document = makeDocument();
  const parts = [
    { key: "red", css: ".x{color:red}" },
    { key: "blue", css: ".x{color:blue}" },
  ];
  const first = styleSheets([...parts, parts[0]!], document);
  assert.equal(first[0], first[2]);
  assert.equal(first[0], styleSheets(parts, document)[0]);
  assert.notEqual(first[0], styleSheets(parts, makeDocument())[0]);
  assert.notEqual(first[0], styleSheets([{ key: "red", css: "changed" }], document)[0]);
});
