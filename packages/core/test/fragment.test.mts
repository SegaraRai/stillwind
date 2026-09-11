import assert from "node:assert/strict";
import test from "node:test";

import * as astro from "../src/runtime/astro.ts";
import { createFragmentRuntime } from "../src/runtime/fragment.ts";
import * as react from "../src/runtime/react.ts";
import * as svelte from "../src/runtime/svelte.ts";

void test("react merges fragments, arrays, and falsy values in argument order", () => {
  const first = { [react.stillwindClassKey]: "_a", "data-state": "ready" };
  const second = { [react.stillwindClassKey]: "_b" };
  assert.deepEqual(
    react.stillwindMerge(first, null, [second, false, [[{ [react.stillwindClassKey]: "_c" }]]]),
    {
      className: "_a _b _c",
      "data-state": "ready",
    },
  );
  assert.deepEqual(react.stillwindMerge(), {});
  assert.deepEqual(react.stillwindMerge(undefined, false), {});
});

void test("later props and style payloads win without leaking symbol keys", () => {
  const merged = react.stillwindMerge(
    {
      [react.stillwindClassKey]: "_a",
      [react.stillwindStyleKey]: { "--x": "1", "--y": "1" },
      title: "a",
    },
    { [react.stillwindStyleKey]: { "--y": "2" }, title: "b" },
  );
  assert.deepEqual(merged, { className: "_a", title: "b", style: { "--y": "2" } });
  assert.deepEqual(Object.getOwnPropertySymbols(merged), []);
});

void test("string-style frameworks serialize the payload and pass strings through", () => {
  assert.deepEqual(
    svelte.stillwindMerge(
      { [svelte.stillwindClassKey]: "_a", [svelte.stillwindStyleKey]: { "--x": "1", "--y": "2" } },
      { [svelte.stillwindStyleKey]: "--y:3" },
    ),
    { class: "_a", style: "--y:3" },
  );
  assert.deepEqual(
    astro.stillwindMerge({
      [astro.stillwindClassKey]: "_z",
      [astro.stillwindStyleKey]: { "--x": "1" },
    }),
    { class: "_z", style: "--x:1" },
  );
});

void test("toFragment converts materialized bases and is idempotent", () => {
  const fragment = react.stillwindToFragment({
    className: "_base",
    style: { "--x": "1" },
    "data-a": "1",
  });
  assert.equal(fragment[react.stillwindClassKey], "_base");
  assert.deepEqual(fragment[react.stillwindStyleKey], { "--x": "1" });
  assert.equal(fragment["data-a"], "1");
  assert.equal(react.stillwindToFragment(fragment), fragment);
  assert.deepEqual(react.stillwindMerge(fragment, { [react.stillwindClassKey]: "_own" }), {
    className: "_base _own",
    "data-a": "1",
    style: { "--x": "1" },
  });
  const svelteFragment = svelte.stillwindToFragment({ class: "_base", style: "--x:1" });
  assert.equal(svelteFragment[svelte.stillwindClassKey], "_base");
  assert.equal(svelteFragment[svelte.stillwindStyleKey], "--x:1");
});

void test("development checks reject foreign fragments and unconverted props objects", () => {
  const foreign = createFragmentRuntime({ classProperty: "className", styleFormat: "object" });
  assert.throws(
    () => react.stillwindMerge({ [foreign.stillwindClassKey]: "_x" }),
    /different runtime instance/,
  );
  assert.throws(() => react.stillwindMerge({ className: "_x" }), /stillwindToFragment/);
  assert.throws(() => svelte.stillwindMerge({ class: "_x" }), /stillwindToFragment/);
});

void test("class concatenation keeps falsy and non-string fragments empty", () => {
  assert.deepEqual(
    react.stillwindMerge(
      { [react.stillwindClassKey]: "" },
      { [react.stillwindClassKey]: undefined },
    ),
    {},
  );
  assert.deepEqual(react.stillwindMerge({ title: "only" }), { title: "only" });
});
