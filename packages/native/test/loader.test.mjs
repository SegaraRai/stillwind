import assert from "node:assert/strict";
import test from "node:test";

void test("loads the current platform package through the public loader", async () => {
  const binding = await import("../index.js");
  assert.equal(typeof binding.compileReact, "function");
  assert.equal(typeof binding.compileReactProject, "function");
});

void test("native sessions retain shared rules and replace CSS revisions", async () => {
  const { NativeDesignSystemLoader, NativeProjectSession } = await import("../index.js");
  const loader = new NativeDesignSystemLoader();
  assert.deepEqual(JSON.parse(loader.provide("opaque root", "@import 'tokens';")), {
    kind: "needFile",
    importer: "opaque root",
    specifier: "tokens",
  });
  assert.equal(
    JSON.parse(loader.provide("opaque child", "@theme { --color-brand: red; }")).kind,
    "complete",
  );
  const session = new NativeProjectSession(loader, true);
  const source = "import { style } from 'stillwind/react'; const x = style('bg-brand');";
  const result = JSON.parse(
    session.compile(
      JSON.stringify([
        { path: "a.tsx", source },
        { path: "b.tsx", source },
      ]),
    ),
  );
  assert.equal(result.modules.length, 2);
  assert.ok(result.modules.every((module) => module.diagnostics.length === 0));
  const before = session.css();
  assert.equal(session.remove("a.tsx"), false);
  assert.equal(session.css(), before);
  const next = new NativeDesignSystemLoader();
  next.provide("root", "@theme { --color-brand: blue; }");
  assert.equal(JSON.parse(session.reload(next)).cssChanged, true);
  assert.notEqual(session.css(), before);
  assert.equal(session.remove("b.tsx"), true);
});
