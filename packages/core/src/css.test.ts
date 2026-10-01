import { expect, test } from "vite-plus/test";

import { cssText, stylesheetLinks, styleSheets } from "./css.ts";

test("manifest selections preserve occurrence positions across reverse requests", () => {
  const manifest = {
    version: 1 as const,
    revision: "build",
    parts: [
      { id: "red:first", href: "/red.css" },
      { id: "blue", href: "/blue.css" },
      { id: "red:last", href: "/red.css" },
    ],
    modules: {
      a: ["red:first", "red:last"],
      b: ["blue"],
    },
  };
  expect(stylesheetLinks(manifest, ["b", "a"])).toStrictEqual(
    manifest.parts.map((part, order) => ({ ...part, order })),
  );
  expect(() => stylesheetLinks(manifest, ["missing"])).toThrow(/Unknown Stillwind/);
  expect(
    cssText([
      { key: "red", css: "red" },
      { key: "blue", css: "blue" },
      { key: "red", css: "red" },
    ]),
  ).toBe("redbluered");
});

test("constructed sheets share content within a Document and retain repeated positions", () => {
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
  expect(first[0]).toBe(first[2]);
  expect(first[0]).toBe(styleSheets(parts, document)[0]);
  expect(first[0]).not.toBe(styleSheets(parts, makeDocument())[0]);
  expect(first[0]).not.toBe(styleSheets([{ key: "red", css: "changed" }], document)[0]);
});
