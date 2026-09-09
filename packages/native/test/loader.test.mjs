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
  assert.deepEqual(JSON.parse(loader.provide(0, "opaque root", "@import 'tokens';")), {
    kind: "needFile",
    requests: [{ id: 1, importer: "opaque root", specifier: "tokens" }],
  });
  assert.equal(
    JSON.parse(loader.provide(1, "opaque child", "@theme { --color-brand: red; }")).kind,
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
  next.provide(0, "root", "@theme { --color-brand: blue; }");
  assert.equal(JSON.parse(session.reload(next)).cssChanged, true);
  assert.notEqual(session.css(), before);
  assert.equal(session.remove("b.tsx"), true);
});

void test("native loader accepts partial and reversed batches", async () => {
  const { NativeDesignSystemLoader } = await import("../index.js");
  const loader = new NativeDesignSystemLoader();
  const first = JSON.parse(
    loader.provideMany([{ id: 0, name: "root", content: "@import 'a'; @import 'b';" }]),
  );
  assert.deepEqual(
    first.requests.map((request) => request.id),
    [1, 2],
  );
  const next = JSON.parse(loader.provide(2, "b", "@import 'c';"));
  assert.deepEqual(
    next.requests.map((request) => request.id),
    [1, 3],
  );
  const done = JSON.parse(
    loader.provideMany([
      { id: 3, name: "c", content: "@theme { --color-child: green; }" },
      { id: 1, name: "a", content: "@theme { --color-brand: red; }" },
    ]),
  );
  assert.equal(done.kind, "complete");
  assert.deepEqual(new Set(done.sources), new Set(["root", "a", "b", "c"]));
  assert.match(loader.generateReactTypes(), /brand/);
  assert.throws(() => loader.provideMany([]), /after completion/);
});
