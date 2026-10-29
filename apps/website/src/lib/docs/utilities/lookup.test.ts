import { expect, test } from "vite-plus/test";

import { documentationCompiler } from "../reference/compiler";
import { lookupUtilities, matchCaption, parseDeclarations } from "./lookup";

const { query } = await documentationCompiler();
const lookup = (input: string, limit?: number) =>
  lookupUtilities(query, input, limit).matches.map(({ utility, source, from, relation }) => ({
    utility,
    source,
    from,
    ...(relation ? { relation } : {}),
  }));

test("resolves a utility to its CSS and lists utilities that start with it", () => {
  const result = lookupUtilities(query, "flex", 3);
  expect(result.matches.map(({ utility, source }) => [utility, source])).toEqual([
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
      utility: "inset-s-4",
      source: "utility",
      from: "start-4",
      relation: "same",
    },
  ]);
  expect(matchCaption(lookupUtilities(query, "start-4").matches[0]!)).toBe("Same as start-4");
  expect(lookup("mt-4 mr-4 mb-4 ml-4", 1)).toEqual([
    {
      utility: "m-4",
      source: "utility",
      from: "mt-4 mr-4 mb-4 ml-4",
    },
  ]);
  expect(lookup("pt-4 pb-4").map(({ utility }) => utility)).toEqual(["pt-4", "pb-4"]);
});

test("follows a canonical spelling with different declarations with the spelling as written", () => {
  const [canonical, written] = lookupUtilities(query, "p-[16px]", 2).matches;
  expect(canonical).toMatchObject({
    utility: "p-4",
    from: "p-[16px]",
  });
  expect(canonical!.relation).toBeUndefined();
  expect(matchCaption(canonical!)).toBe("Canonical spelling of p-[16px]");
  expect(written).toMatchObject({
    utility: "p-[16px]",
    relation: "written",
  });
  expect(written!.css).toContain("padding: 16px");
  expect(matchCaption(written!)).toBe("As written");
});

test("converts CSS declarations to canonical utilities, including arbitrary properties", () => {
  expect(lookup("& { display: flex; padding: 16px; }")).toEqual([
    {
      utility: "flex p-4",
      source: "css",
      from: "& { display: flex; padding: 16px; }",
    },
    {
      utility: "flex",
      source: "css",
      from: "display: flex",
    },
    {
      utility: "p-4",
      source: "css",
      from: "padding: 16px",
    },
  ]);
  expect(lookup("background-color: oklch(62.3% 0.214 259.815)")).toEqual([
    {
      utility: "bg-blue-500",
      source: "css",
      from: "background-color: oklch(62.3% 0.214 259.815)",
    },
  ]);
  expect(lookup("hyphens: manual")).toEqual([
    {
      utility: "[hyphens:manual]",
      source: "css",
      from: "hyphens: manual",
    },
  ]);
});

test("reads a variant as a utility unless a declaration names a utility", () => {
  expect(lookup("hover:underline", 1)).toEqual([
    {
      utility: "hover:underline",
      source: "utility",
      from: undefined,
    },
  ]);
});

test("returns no matches for empty input or text that is neither CSS nor a utility", () => {
  expect(lookupUtilities(query, "  ").matches).toEqual([]);
  expect(lookupUtilities(query, "no such utility").matches).toEqual([]);
});

test("parses declarations with or without a surrounding rule", () => {
  expect(parseDeclarations("Display: flex\n--Gap:  1rem;")).toEqual([
    ["display", "flex"],
    ["--Gap", "1rem"],
  ]);
  expect(parseDeclarations("p-4")).toBeUndefined();
  expect(parseDeclarations("display: flex; p-4")).toBeUndefined();
});
