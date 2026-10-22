import { expect, test } from "vite-plus/test";

import { searchUtilities, type UtilityRecord } from "./search";

const records: UtilityRecord[] = [
  { name: "inline-flex", css: "& { display: inline-flex; }", marker: false },
  { name: "flex-1", css: "& { flex: 1; }", marker: false },
  { name: "flex", css: "& { display: flex; }", marker: false },
  { name: "p-4", css: "& { padding: calc(var(--spacing) * 4); /* 1rem = 16px */ }", marker: false },
  { name: "group", css: "", marker: true },
];

test("ranks exact utilities before prefixes and substring matches without changing the catalog", () => {
  expect(searchUtilities(records, "utility", " flex ").records.map(({ name }) => name)).toEqual([
    "flex",
    "flex-1",
    "inline-flex",
  ]);
  expect(records[0]!.name).toBe("inline-flex");
});

test("matches every CSS search term and keeps partial property/value matches visible", () => {
  expect(
    searchUtilities(records, "css", "{ DISPLAY: flex; }").records.map(({ name }) => name),
  ).toEqual(["inline-flex", "flex"]);
  expect(searchUtilities(records, "css", "padding 16px").records.map(({ name }) => name)).toEqual([
    "p-4",
  ]);
  expect(searchUtilities(records, "css", "display padding").total).toBe(0);
});

test("limits rendered results while reporting all matches and retaining marker entries", () => {
  const result = searchUtilities(records, "utility", "", 2);
  expect(result.total).toBe(records.length);
  expect(result.records).toHaveLength(2);
  expect(searchUtilities(records, "utility", "group").records[0]).toMatchObject({
    marker: true,
    css: "",
  });
  expect(searchUtilities(records, "css", "group").total).toBe(0);
});
