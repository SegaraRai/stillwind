import assert from "node:assert/strict";
import test from "node:test";

import * as astro from "../dist/runtime/astro.js";
import * as react from "../dist/runtime/react.js";
import * as svelte from "../dist/runtime/svelte.js";
import { createFragmentRuntime } from "../src/runtime/fragment.ts";

void test("react merges fragments, arrays, and falsy values in argument order", () => {
  const frozenBase = Object.freeze({ className: "a" });
  const frozenNext = Object.freeze({ className: "b" });
  const delta = Object.freeze({ [react.stillwindClassKey]: "own", "data-state": "ready" });
  assert.deepEqual(
    react.stillwindMerge(
      react.stillwindToFragment(frozenBase),
      react.stillwindToFragment(frozenNext),
      delta,
    ),
    {
      className: "a b own",
      "data-state": "ready",
    },
  );
  assert.deepEqual(
    react.stillwindMerge(
      react.stillwindToFragment(frozenBase),
      react.stillwindToFragment(frozenNext),
    ),
    { className: "a b" },
  );
  assert.deepEqual(
    Object.getOwnPropertySymbols(react.stillwindMerge(react.stillwindToFragment(frozenBase))),
    [],
  );
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

void test("custom properties merge independently and coexist with generated styles", () => {
  for (const runtime of [react, svelte, astro]) {
    const key = runtime.stillwindCustomPropertiesKey;
    const result = runtime.stillwindMerge(
      { [key]: { "--progress": "0", "--image": "url(image.png)" } },
      false,
      [{ [key]: { "--progress": "0.5" } }],
      { [runtime.stillwindStyleKey]: { "view-transition-name": "card" } },
    );
    assert.deepEqual(
      result.style,
      runtime === react
        ? { "view-transition-name": "card", "--progress": "0.5", "--image": "url(image.png)" }
        : "view-transition-name:card;--progress:0.5;--image:url(image.png)",
    );
    assert.deepEqual(Object.getOwnPropertySymbols(result), []);
    const fragment = { [key]: { "--x": "1" } };
    assert.equal(runtime.stillwindToFragment(fragment), fragment);
  }
});

void test("CSS values preserve strings and zero, encode boolean switches and quote URL objects", () => {
  assert.equal(react.stillwindCssValue(0), "0");
  assert.equal(react.stillwindCssValue(false), "initial");
  assert.equal(react.stillwindCssValue(true), " ");
  assert.equal(react.stillwindCssValue("calc(1px + 2px)"), "calc(1px + 2px)");
  assert.equal(
    react.stillwindCssValue(new URL("https://example.com/image.png")),
    'url("https://example.com/image.png")',
  );
});

void test("boolean switches retain whitespace and initial in framework style props", () => {
  for (const runtime of [react, svelte, astro]) {
    for (const enabled of [true, false]) {
      const value = runtime.stillwindCssValue(enabled);
      const props = runtime.stillwindMerge({
        [runtime.stillwindCustomPropertiesKey]: { "--enabled": value },
      });
      assert.deepEqual(
        props.style,
        runtime === react
          ? { "--enabled": enabled ? " " : "initial" }
          : enabled
            ? "--enabled: "
            : "--enabled:initial",
      );
    }
  }
});

void test("style value conversion matches merge serialization", () => {
  for (const styleFormat of ["object", "string"] as const) {
    for (const classProperty of ["class", "className"]) {
      const runtime = createFragmentRuntime({ classProperty, styleFormat });
      for (const value of [
        undefined,
        null,
        false,
        true,
        0,
        1,
        "",
        "a b",
        {},
        { "--x": 1 },
        ["a"],
        Symbol("value"),
      ]) {
        assert.deepEqual(
          runtime.stillwindStyleValue(value),
          runtime.stillwindMerge({ [runtime.stillwindStyleKey]: value }).style,
        );
      }
    }
  }
});
