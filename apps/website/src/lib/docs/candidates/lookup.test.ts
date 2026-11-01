import { expect, test } from "vite-plus/test";

import { documentationCompiler } from "../reference/compiler";
import { lookupCandidates, matchCaption, parseDeclarations } from "./lookup";

const { query } = await documentationCompiler();
const lookup = (input: string, limit?: number) =>
  lookupCandidates(query, input, limit).matches.map(({ candidate, source, from, relation }) => ({
    candidate,
    source,
    from,
    ...(relation ? { relation } : {}),
  }));

test("resolves a utility to its CSS and lists utilities that start with it", () => {
  const result = lookupCandidates(query, "flex", 3);
  expect(result.matches.map(({ candidate, source }) => [candidate, source])).toEqual([
    ["flex", "utility"],
    ["flex-1", "prefix"],
    ["flex-auto", "prefix"],
  ]);
  expect(result.matches[0]!.css).toContain("display: flex");
  expect(result.prefixTotal).toBeGreaterThan(2);
});

test("converts a utility and an arbitrary value to their canonical spellings", () => {
  expect(lookup("start-4", 1)).toEqual([
    {
      candidate: "inset-s-4",
      source: "utility",
      from: "start-4",
      relation: "same",
    },
  ]);
  expect(matchCaption(lookupCandidates(query, "start-4").matches[0]!)).toBe("Same as start-4");
  expect(lookup("mt-4 mr-4 mb-4 ml-4", 1)).toEqual([
    {
      candidate: "m-4",
      source: "utility",
      from: "mt-4 mr-4 mb-4 ml-4",
    },
  ]);
  expect(lookup("pt-4 pb-4").map(({ candidate }) => candidate)).toEqual(["pt-4", "pb-4"]);
});

test("follows a canonical spelling with different declarations with the spelling as written", () => {
  const [canonical, written] = lookupCandidates(query, "p-[16px]", 2).matches;
  expect(canonical).toMatchObject({
    candidate: "p-4",
    from: "p-[16px]",
  });
  expect(canonical!.relation).toBeUndefined();
  expect(matchCaption(canonical!)).toBe("Canonical spelling of p-[16px]");
  expect(written).toMatchObject({
    candidate: "p-[16px]",
    relation: "written",
  });
  expect(written!.css).toContain("padding: 16px");
  expect(matchCaption(written!)).toBe("As written");
});

test("converts CSS declarations to canonical utilities, including arbitrary properties", () => {
  expect(lookup("& { display: flex; padding: 16px; }")).toEqual([
    {
      candidate: "flex p-4",
      source: "css",
      from: "& { display: flex; padding: 16px; }",
    },
    {
      candidate: "flex",
      source: "css",
      from: "display: flex",
    },
    {
      candidate: "p-4",
      source: "css",
      from: "padding: 16px",
    },
  ]);
  expect(lookup("background-color: oklch(62.3% 0.214 259.815)")).toEqual([
    {
      candidate: "bg-blue-500",
      source: "css",
      from: "background-color: oklch(62.3% 0.214 259.815)",
    },
  ]);
  expect(lookup("hyphens: manual")).toEqual([
    {
      candidate: "[hyphens:manual]",
      source: "css",
      from: "hyphens: manual",
    },
  ]);
});

test("reads a variant as a utility unless a declaration names a utility", () => {
  expect(lookup("hover:underline", 1)).toEqual([
    {
      candidate: "hover:underline",
      source: "utility",
      from: undefined,
    },
  ]);
});

test("finds a variant by its name or by part of its CSS", () => {
  const first = (input: string) => lookupCandidates(query, input).matches[0]?.candidate;
  for (const input of [
    "hover",
    "hover:",
    ":hover",
    "&:hover",
    "HOVER",
    "@media (hover: hover)",
    "@media (hover:hover)",
    "media hover",
    "hover: hover",
  ]) {
    expect(first(input)).toBe("hover:");
  }
  expect(lookupCandidates(query, ":focus").variantTotal).toBeGreaterThan(5);
  expect(first(":focus")).toBe("focus:");
  expect(first(">= 48rem")).toBe("md:");
});

test("resolves an exact variant that completion does not list", () => {
  expect(lookupCandidates(query, "data-active").matches[0]).toMatchObject({
    candidate: "data-active:",
    source: "variant",
  });
  expect(lookupCandidates(query, "hover:focus").matches[0]!.candidate).toBe("hover:focus:");
});

test("converts a variant to its canonical spelling", () => {
  const [match] = lookupCandidates(query, "[&:focus]").matches;
  expect(match).toMatchObject({
    candidate: "focus:",
    source: "variant",
    from: "[&:focus]:",
    relation: "same",
  });
  expect(match!.css).toContain("&:focus {");
  expect(matchCaption(match!)).toBe("Same as [&:focus]:");
  expect(lookupCandidates(query, "data-[active]").matches[0]!.candidate).toBe("data-active:");
});

test("keeps the spelling of a variant that has no equivalent canonical spelling", () => {
  const candidate = lookupCandidates(query, "[&:hover]").matches[0]!;
  expect(candidate).toMatchObject({
    candidate: "[&:hover]:",
    source: "variant",
  });
  expect(matchCaption(candidate)).toBe("Variant");
});

test("returns results for input whose utility reading is not valid CSS", () => {
  for (const input of ["[:focus]", "[@media(hover:hover)]"]) {
    expect(lookupCandidates(query, input).matches[0]!.source).toBe("variant");
  }
});

test("keeps the utility reading of an arbitrary property", () => {
  expect(lookup("[color:red]").map(({ source }) => source)).not.toContain("variant");
});

test("marks where a variant places the utility declarations", () => {
  const { css, source } = lookupCandidates(query, "hover").matches[0]!;
  expect(source).toBe("variant");
  expect(css).toContain("@media (hover: hover)");
  expect(css).toContain("@media (hover: hover) {\n    /* utility declarations */\n  }");
});

test("keeps the empty rule of a variant only when at-rules alone enclose it", () => {
  const css = (input: string) => lookupCandidates(query, input).matches[0]!.css.trimEnd();
  expect(css("first")).toBe("&:first-child {\n  /* utility declarations */\n}");
  expect(css("dark")).toBe(
    "@media (prefers-color-scheme: dark) {\n  & {\n    /* utility declarations */\n  }\n}",
  );
  expect(css("not-hover")).toBe(
    "&:not(:hover) {\n  /* utility declarations */\n}\n\n" +
      "@media not (hover: hover) {\n  & {\n    /* utility declarations */\n  }\n}",
  );
});

test("limits variants so that prefix matches keep the remaining places", () => {
  const { matches, variantTotal } = lookupCandidates(query, "in", 10, 2);
  expect(matches.filter(({ source }) => source === "variant")).toHaveLength(2);
  expect(matches.filter(({ source }) => source === "prefix")).toHaveLength(8);
  expect(variantTotal).toBeGreaterThan(2);
});

test("matches a term of one character against variant names only", () => {
  expect(lookupCandidates(query, "&").matches).toEqual([]);
});

test("returns no matches for empty input or text that is neither CSS nor a utility", () => {
  expect(lookupCandidates(query, "  ").matches).toEqual([]);
  expect(lookupCandidates(query, "no such utility").matches).toEqual([]);
});

test("parses declarations with or without a surrounding rule", () => {
  expect(parseDeclarations("Display: flex\n--Gap:  1rem;")).toEqual([
    ["display", "flex"],
    ["--Gap", "1rem"],
  ]);
  expect(parseDeclarations("p-4")).toBeUndefined();
  expect(parseDeclarations("display: flex; p-4")).toBeUndefined();
});
