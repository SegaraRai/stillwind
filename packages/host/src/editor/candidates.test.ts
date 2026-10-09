import { NativeDesignSystemLoader } from "@stillwind/native";
import { expect, test } from "vite-plus/test";

import { CandidateQueries } from "./candidates.ts";

test("completion caches share the catalog for a repeated prefix", () => {
  const loader = new NativeDesignSystemLoader({});
  expect(loader.provide(0, "theme.css", "@utility block { display:block; }").kind).toBe("complete");
  const candidates = new CandidateQueries(loader.editorQuery());
  const labels = candidates.complete("blo");
  expect(labels).toContain("block");
  expect(candidates.complete("blo")).toBe(labels);
});
