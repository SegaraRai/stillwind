import { NativeDesignSystemLoader } from "@stillwind/native";
import { expect, test } from "vite-plus/test";

import { CandidateQueries, markerDocumentation } from "./candidates.ts";

test("completion caches share the catalog for a repeated prefix", () => {
  const loader = new NativeDesignSystemLoader({});
  expect(loader.provide(0, "theme.css", "@utility block { display:block; }").kind).toBe("complete");
  const candidates = new CandidateQueries(loader.editorQuery());
  const labels = candidates.complete("blo");
  expect(labels.map(({ label }) => label)).toContain("block");
  expect(candidates.complete("blo")).toBe(labels);
});

test("marker documentation names the class and groups its origins", () => {
  expect(
    markerDocumentation({
      class: "card",
      origins: [
        { kind: "declared" },
        { kind: "source" },
        { kind: "utility", name: "card-title" },
        { kind: "utility", name: "card-body" },
        { kind: "variant", name: "in-card" },
      ],
    }),
  ).toBe(
    [
      "Adds the marker class `card` to the element as written.",
      "A `@stillwind-marker` statement declares this class.",
      "Selectors in the `card-title` utility, the `card-body` utility, and the `in-card` variant select this class.",
      "Selectors in plugin CSS select this class.",
    ].join("\n\n"),
  );
});
