import assert from "node:assert/strict";
import test from "node:test";

void test("loads the current platform package through the public loader", async () => {
  const binding = await import("../index.js");
  assert.equal(typeof binding.compileReact, "function");
  assert.equal(typeof binding.compileReactProject, "function");
});

void test("session reload applies code generation options independently of CSS definitions", async () => {
  const { NativeDesignSystemLoader, NativeProjectSession } = await import("../index.js");
  const load = (classNamespace) => {
    const loader = new NativeDesignSystemLoader(
      JSON.stringify({ classIdentity: "module", classNamespace }),
    );
    const result = JSON.parse(
      loader.provide(0, "root", "@utility flex { display:flex; } @stillwind-slot utilities;"),
    );
    assert.equal(result.kind, "complete");
    return loader;
  };
  const session = new NativeProjectSession(load("first"), true);
  const sources = JSON.stringify([
    {
      path: "button.tsx",
      source: "import { style } from 'stillwind/react'; const x = style('flex');",
    },
  ]);
  const before = JSON.parse(session.compile(sources));
  const beforeCss = session.css();
  const update = JSON.parse(session.reload(load("second")));
  assert.equal(update.cssChanged, true);
  assert.deepEqual(update.transformedModules, ["button.tsx"]);
  assert.notEqual(session.css(), beforeCss);
  const after = JSON.parse(session.compile(sources));
  assert.notEqual(before.modules[0].code, after.modules[0].code);
  assert.deepEqual(after.modules[0].diagnostics, []);
});

void test("native sessions retain shared rules and replace CSS revisions", async () => {
  const { NativeDesignSystemLoader, NativeProjectSession } = await import("../index.js");
  const loader = new NativeDesignSystemLoader();
  assert.deepEqual(
    JSON.parse(
      loader.provide(
        0,
        "opaque root",
        "@import 'tokens'; @utility bg-brand {background-color:var(--color-brand)} @stillwind-slot utilities;",
      ),
    ),
    {
      kind: "needFile",
      requests: [{ id: 1, importer: "opaque root", specifier: "tokens" }],
    },
  );
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
  assert.ok(before.length > 0);
  assert.equal(session.remove("a.tsx"), false);
  assert.equal(session.css(), before);
  const next = new NativeDesignSystemLoader();
  next.provide(
    0,
    "root",
    "@theme { --color-brand: blue; } @utility bg-brand {background-color:var(--color-brand)} @stillwind-slot utilities;",
  );
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
      {
        id: 1,
        name: "a",
        content: "@theme { --color-brand: red; } @utility brand {color:var(--color-brand)}",
      },
    ]),
  );
  assert.equal(done.kind, "complete");
  assert.deepEqual(new Set(done.sources), new Set(["root", "a", "b", "c"]));
  assert.match(loader.generateReactTypes(), /brand/);
  assert.throws(() => loader.provideMany([]), /after completion/);
});

void test("native watch reload reports compilation errors without publishing a partial revision", async () => {
  const { NativeDesignSystemLoader, NativeProjectSession } = await import("../index.js");
  const initial = new NativeDesignSystemLoader();
  initial.provide(0, "theme", "@utility brand { color:red; } @stillwind-slot utilities;");
  const session = new NativeProjectSession(initial, true);
  const source =
    "import { style } from 'stillwind/react'; const motion = style.animation({ from: 'brand' }); const x = style(motion(''));";
  const output = JSON.parse(session.compile(JSON.stringify([{ path: "motion.tsx", source }])));
  assert.deepEqual(output.modules[0].diagnostics, []);
  const before = session.css();
  assert.ok(before.length > 0);
  const invalid = new NativeDesignSystemLoader();
  invalid.provide(0, "theme", "");
  assert.throws(() => session.reload(invalid), /CSS reload compilation failed.*SH001/s);
  assert.equal(session.css(), before);
  const recovered = new NativeDesignSystemLoader();
  recovered.provide(0, "theme", "@utility brand { color:blue; } @stillwind-slot utilities;");
  assert.equal(JSON.parse(session.reload(recovered)).cssChanged, true);
  assert.notEqual(session.css(), before);
});

void test("native sessions join supplied type facts with another module's asset metadata", async () => {
  const { NativeDesignSystemLoader, NativeProjectSession, summarizeReact } =
    await import("../index.js");
  const loader = new NativeDesignSystemLoader();
  loader.provide(0, "theme", "");
  const producer = {
    path: "producer.tsx",
    source: "import { style } from 'stillwind/react'; export const marker = style.ref('shared');",
  };
  const consumer = {
    path: "consumer.tsx",
    source:
      "import { style } from 'stillwind/react'; import { marker } from './producer'; const props = style(marker);",
  };
  const summary = JSON.parse(summarizeReact(producer.source, producer.path));
  const start = consumer.source.lastIndexOf("marker");
  const session = new NativeProjectSession(loader, true);
  const without = JSON.parse(session.compile(JSON.stringify([consumer, producer])));
  assert.ok(without.modules[0].diagnostics.length > 0);
  const withFacts = JSON.parse(
    session.compile(
      JSON.stringify([
        {
          ...consumer,
          type_facts: [{ start, end: start + "marker".length, value: summary.exports.marker }],
        },
        producer,
      ]),
    ),
  );
  assert.deepEqual(
    withFacts.modules.map((module) => module.path),
    [consumer.path, producer.path],
  );
  assert.ok(
    withFacts.modules.every((module) => module.diagnostics.length === 0),
    JSON.stringify(withFacts),
  );
  assert.match(withFacts.modules[0].code, /s_shared_/);
  assert.doesNotMatch(withFacts.modules[0].code, /style\(marker\)/);
});
