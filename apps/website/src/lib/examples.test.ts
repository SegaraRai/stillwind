import { expect, it } from "vite-plus/test";

import { createExamples } from "./examples";

it("builds the displayed findings, canonical utilities, and CSS with the compiler", async () => {
  const examples = await createExamples();
  expect(examples.unknown).toMatchObject({
    code: "unknown-utility",
    severity: "error",
  });
  expect(examples.restrictions.map((report) => report.code).sort()).toEqual([
    "no-arbitrary-breakpoints",
    "no-arbitrary-colors",
  ]);
  expect(examples.restrictions.every((report) => report.severity === "error")).toBe(true);
  expect(examples.unchecked.split(" ")[examples.unknown.utilityIndex]).toBe("bg-inlk");
  expect(
    examples.restrictions
      .map((report) => examples.restricted.split(" ")[report.utilityIndex])
      .sort(),
  ).toEqual(["bg-[#d9e2f0]", "min-[713px]:px-6"]);
  expect(examples.ordered.split(" ")).toContain("px-4");
  expect(examples.compiled).toBe(examples.ordered);
  expect(examples.canonical).toHaveLength(1);
  expect(examples.canonical[0]).toMatchObject({
    code: "prefer-canonical-utilities",
    severity: "warning",
  });
  expect(examples.unordered.split(" ")[examples.canonical[0]!.utilityIndex]).toBe("px-[16px]");
  expect(examples.canonicalized).toBe("py-2 text-paper px-4 bg-ink rounded-md");
  expect(examples.css).toContain("padding-inline:");
  expect(examples.css).toContain("background-color:");
  expect(examples.compiledCode).toContain("className:");
  expect(examples.compiledCode).toContain('"' + examples.cssClassName + '"');
  expect(examples.css).toContain("." + examples.cssClassName + " {");
});
