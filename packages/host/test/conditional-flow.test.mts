import assert from "node:assert/strict";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";
import { runInNewContext } from "node:vm";

import { TraceMap, originalPositionFor } from "@jridgewell/trace-mapping";
import { NativeDesignSystemLoader, NativeProjectSession } from "@stillwind/native";

import * as runtime from "../../core/src/runtime/react.ts";
import * as svelteRuntime from "../../core/src/runtime/svelte.ts";
import { compileModule } from "../src/compiler.ts";
import { moduleSourceMap } from "../src/source-map.ts";
import { TypeScriptFactsService } from "../src/type-facts.ts";

async function compile(source: string) {
  const root = path.dirname(fileURLToPath(import.meta.url));
  const loader = new NativeDesignSystemLoader();
  loader.provide(
    0,
    "design.css",
    "@utility red {color:red} @utility blue {color:blue} @utility green {color:green} @stillwind-slot utilities;",
  );
  const session = new NativeProjectSession(loader, false);
  const facts = new TypeScriptFactsService(root);
  try {
    const output = await compileModule(
      session,
      () => facts,
      `${source}\nglobalThis.result = { props, events };`,
      path.join(root, "entry.ts"),
      root,
      {
        resolve: async (specifier, importer) => ({
          id: path.resolve(path.dirname(importer), `${specifier}.ts`),
        }),
        read: async () => {
          throw new Error("unexpected dependency");
        },
        watch: () => {},
        warn: () => {},
      },
    );
    assert.ok(output);
    return output;
  } finally {
    facts.close();
  }
}

function evaluate(code: string, context: Record<string, unknown>) {
  const sandbox: Record<string, unknown> = { ...runtime, ...context };
  runInNewContext(code.replace(/^import\s+[^;]+;\s*/gm, ""), sandbox);
  return sandbox.result as {
    props: Record<string, unknown>;
    events: string[];
  };
}

void test("guarded view transitions preserve short-circuiting through type collection", async () => {
  const output = await compile(
    [
      "import {style} from 'stillwind/react'; const events = [];",
      "const enabled = Boolean(globalThis.enabled);",
      "const selected = style.viewTransition({old: 'red', new: 'blue'});",
      "const base = style.viewTransition({old: 'green'});",
      "const key = () => { events.push('key'); return 'guarded'; };",
      "const props = style('red', enabled && selected.for(key()), base.for('base'));",
    ].join("\n"),
  );
  const disabled = evaluate(output.code!, { enabled: false });
  const enabled = evaluate(output.code!, { enabled: true });
  assert.equal(disabled.events.length, 0);
  assert.equal(enabled.events.join(), "key");
  const baseClasses = String(disabled.props.className).split(" ");
  const enabledClasses = String(enabled.props.className).split(" ");
  assert.equal(baseClasses.length, 1);
  assert.equal(enabledClasses.length, 2);
  assert.ok(enabledClasses.includes(baseClasses[0]));
});

void test("pure conditional calls select one class branch", async () => {
  const output = await compile(
    [
      "import {style} from 'stillwind/react';",
      "const first = () => enabled; const events = [];",
      "const props = style('red', first() ? 'blue' : 'green');",
    ].join("\n"),
  );
  const yes = evaluate(output.code!, { enabled: true }).props;
  const no = evaluate(output.code!, { enabled: false }).props;
  assert.equal(String(yes.className).split(" ").length, 2);
  assert.equal(String(no.className).split(" ").length, 2);
  assert.notEqual(yes.className, no.className);
});

void test("short-circuiting leaves an unselected nested value unread", async () => {
  const output = await compile(
    [
      "import {style} from 'stillwind/react'; const events = [];",
      "const props = style('red', enabled && (second() ? 'blue' : 'green'));",
    ].join("\n"),
  );
  const disabled = evaluate(output.code!, { enabled: false });
  assert.equal(String(disabled.props.className).split(" ").length, 1);
  const enabled = evaluate(output.code!, { enabled: true, second: () => true });
  assert.equal(String(enabled.props.className).split(" ").length, 2);
});

void test("pure attribute and guard calls materialize directly", async () => {
  const output = await compile(
    [
      "import {style} from 'stillwind/react'; const events = [];",
      "const read = (value) => value;",
      "const props = style({ 'aria-busy': read(true) }, read(enabled) ? 'blue' : 'green');",
    ].join("\n"),
  );
  assert.doesNotMatch(output.code!, /stillwindMerge/);
  const { props } = evaluate(output.code!, { enabled: true });
  assert.deepEqual(Object.keys(props), ["className", "aria-busy"]);
  assert.equal(props["aria-busy"], true);
});

void test("direct props and fragment materialization produce the same classes", async () => {
  const direct = await compile(
    [
      "import {style} from 'stillwind/react';",
      "const events = [];",
      "const props = style('red', enabled && (nested ? 'blue' : 'green'));",
    ].join("\n"),
  );
  const fragment = await compile(
    [
      "import {style} from 'stillwind/react';",
      "const base = style('red');",
      "const events = [];",
      "const props = style(base, enabled && (nested ? 'blue' : 'green'));",
    ].join("\n"),
  );
  assert.doesNotMatch(direct.code!, /stillwindMerge/);
  assert.match(fragment.code!, /stillwindMerge\(stillwindToFragment\(\(base\)\)/);
  const bases = { direct: new Set<string>(), fragment: new Set<string>() };
  for (const context of [
    { enabled: false, nested: false },
    { enabled: true, nested: false },
    { enabled: true, nested: true },
  ]) {
    const first = evaluate(direct.code!, { ...context });
    const second = evaluate(fragment.code!, { ...context });
    // Each call owns its class identity, so compare the selected structure.
    const firstClasses = String(first.props.className).split(" ");
    const secondClasses = String(second.props.className).split(" ");
    assert.equal(secondClasses.length, firstClasses.length, JSON.stringify(context));
    assert.equal(secondClasses.length > 1, firstClasses.length > 1, JSON.stringify(context));
    // The unconditional base unit stays first and stable in both representations.
    bases.direct.add(firstClasses[0]);
    bases.fragment.add(secondClasses[0]);
  }
  assert.equal(bases.direct.size, 1);
  assert.equal(bases.fragment.size, 1);
});

void test("fragments stay inside the call expression", async () => {
  const output = await compile(
    [
      "import {style} from 'stillwind/react';",
      "const events = [];",
      "const props = style({ 'aria-busy': true }, enabled ? 'blue' : 'green', nested && 'red');",
    ].join("\n"),
  );
  assert.doesNotMatch(output.code!, /Symbol\(/);
  const nonImport = output
    .code!.split("\n")
    .filter((line) => !line.startsWith("import "))
    .join("\n");
  const keyUses = nonImport.match(/stillwindClassKey/g)?.length ?? 0;
  assert.equal(nonImport.match(/\[stillwindClassKey\]/g)?.length ?? 0, keyUses);
  const { props } = evaluate(output.code!, { enabled: true, nested: false });
  assert.equal(props["aria-busy"], true);
  assert.equal(String(props.className).split(" ").length, 1);
});

void test("custom-property folding materializes object and string framework props", async () => {
  for (const [framework, helpers] of [
    ["react", runtime],
    ["svelte", svelteRuntime],
  ] as const) {
    const output = await compile(
      [
        `import {style} from 'stillwind/${framework}';`,
        "const events = [];",
        "const props = style('red', { '--progress': (events.push('value'), progress) });",
      ].join("\n"),
    );
    assert.doesNotMatch(
      output.code!,
      /stillwindMerge|stillwindCustomPropertiesKey|stillwindClassKey/,
    );
    for (const [progress, expected] of [
      [0, "0"],
      [true, " "],
      [false, "initial"],
      ["calc(1 + 2)", "calc(1 + 2)"],
    ] as const) {
      const { props, events } = evaluate(output.code!, { ...helpers, progress });
      assert.deepEqual(Array.from(events), ["value"]);
      assert.deepEqual(Object.keys(props), [
        framework === "react" ? "className" : "class",
        "style",
      ]);
      if (framework === "svelte") {
        assert.equal(props.style, `--progress:${expected}`);
      } else {
        assert.equal((props.style as Record<string, unknown>)["--progress"], expected);
      }
    }
  }
});

void test("interleaved pure calls fold into one property record", async () => {
  const output = await compile(
    [
      "import {style} from 'stillwind/react'; const events = [];",
      "const read = (value) => value;",
      "const props = style({ '--x': read(1) }, { 'data-value': read(2) }, { '--y': read(3) });",
    ].join("\n"),
  );
  assert.doesNotMatch(output.code!, /stillwindMerge/);
  const { props } = evaluate(output.code!, {});
  assert.equal(props["data-value"], 2);
  assert.deepEqual(Object.entries(props.style as object), [
    ["--x", "1"],
    ["--y", "3"],
  ]);
});

void test("empty branches omit calls while nonempty branches keep guards", async () => {
  for (const branch of [
    "check() ? [] : []",
    "check() && []",
    "check() ? [[], { '--x': 1 }] : []",
  ]) {
    const output = await compile(
      [
        "import {style} from 'stillwind/react'; const events = [];",
        `const props = style(${branch});`,
      ].join("\n"),
    );
    for (const enabled of [false, true]) {
      const { props } = evaluate(
        output.code!,
        branch.includes("--x") ? { check: () => enabled } : {},
      );
      assert.equal(Object.hasOwn(props, "className"), false);
      assert.equal(Object.hasOwn(props, "style"), enabled && branch.includes("--x"));
    }
  }
});

void test("pure folding and empty-branch removal preserve materialized values", async () => {
  for (const [framework, helpers] of [
    ["react", runtime],
    ["svelte", svelteRuntime],
  ] as const) {
    const output = await compile(
      [
        `import {style} from 'stillwind/${framework}';`,
        "const events = []; const read = (value) => value;",
        "const props = style({ '--x': read(x) }, { 'data-value': x }, { '--y': x + 1 }, enabled && [], 'red');",
      ].join("\n"),
    );
    assert.doesNotMatch(output.code!, /stillwindMerge|enabled/);
    const { props } = evaluate(output.code!, { ...helpers, x: 3 });
    assert.deepEqual(Object.keys(props), [
      framework === "react" ? "className" : "class",
      "data-value",
      "style",
    ]);
    assert.equal(props["data-value"], 3);
    if (framework === "svelte") {
      assert.equal(props.style, "--x:3;--y:4");
    } else {
      assert.deepEqual(Object.entries(props.style as object), [
        ["--x", "3"],
        ["--y", "4"],
      ]);
    }
  }
});

void test("macro mutations and execution boundaries fail builds by default", async () => {
  for (const [expression, code] of [
    ["value++", "macro-argument-mutation"],
    ["import('module')", "macro-execution-boundary"],
  ]) {
    await assert.rejects(
      compile(
        [
          "import {style} from 'stillwind/react'; const events = []; let value = 1;",
          `const props = style({ '--x': ${expression} });`,
        ].join("\n"),
      ),
      new RegExp(code),
    );
  }
});

void test("folded expressions map to distinct original occurrences through native output", async () => {
  const position = (text: string, offset: number) => ({
    line: text.slice(0, offset).split("\n").length,
    column: offset - text.lastIndexOf("\n", offset - 1) - 1,
  });
  for (const framework of ["react", "preact", "solid", "vue", "svelte", "astro"]) {
    for (const call of [
      "style({ '--x': same },\r\n { 'data-value': other },\r\n { '--y': same }, 'red')",
      "style({ '--x': other }, { class: [same, [,, '😀日本', same]] }, 'red')",
      "style(check() ? { '--x': first() } : { '--y': last() })",
      "style('red', enabled && 'blue', { class: extra })",
      "style({ '--x': `😀${value}\n日本語` })",
      "style(flag && [], { '--x': remaining })",
    ]) {
      const source = `import {style} from 'stillwind/${framework}';\r\n// 😀 日本語\r\nconst events = [];\r\nconst props = ${call};`;
      const fullSource = `${source}\nglobalThis.result = { props, events };`;
      const output = await compile(source);
      const map = new TraceMap(
        moduleSourceMap(fullSource, output.code!, "/entry.ts", output.mappings, [], ""),
      );
      const expressions = output.mappings.flatMap((mapping) => mapping.expressions);
      assert.ok(expressions.length > 0);
      for (const expression of expressions) {
        assert.equal(expression.exact, true);
        const originalText = fullSource.slice(expression.original_start, expression.original_end);
        assert.equal(
          output.code!.slice(expression.generated_start, expression.generated_end),
          originalText,
        );
        // Every UTF-16 code unit, including surrogate interiors and line breaks.
        for (let offset = 0; offset < originalText.length; offset++) {
          const actual = originalPositionFor(
            map,
            position(output.code!, expression.generated_start + offset),
          );
          const expected = position(fullSource, expression.original_start + offset);
          assert.deepEqual(
            { line: actual.line, column: actual.column },
            expected,
            `${framework}: ${call}: ${offset}`,
          );
        }
      }
      if (call.includes("same")) {
        const occurrences = expressions.filter(
          (expression) =>
            fullSource.slice(expression.original_start, expression.original_end) === "same",
        );
        assert.equal(occurrences.length, 2);
        assert.notEqual(occurrences[0].original_start, occurrences[1].original_start);
        assert.ok(
          expressions.some(
            (expression, index) =>
              index > 0 && expression.original_start < expressions[index - 1].original_start,
          ),
        );
      }
      if (call.includes("flag")) {
        assert.ok(
          expressions.every(
            (expression) =>
              fullSource.slice(expression.original_start, expression.original_end) !== "flag",
          ),
        );
      }
    }
  }
});

void test("external class arrays flatten while spread keeps iterable semantics", async () => {
  for (const [expression, expected] of [
    ['["first", [,, "日本😀", ["last"]]]', "first 日本😀 last"],
    ['[..."ab"]', "a b"],
    ['[[...new Set(["a", "b"])], "c"]', "a b c"],
    ['[enabled ? ["a", "b"] : [], "c"]', "a b c"],
    ['[("ignored", "kept"), "last"]', "kept last"],
    ["[[], [,,]]", undefined],
  ] as const) {
    const output = await compile(
      `import {style} from 'stillwind/react'; const events = []; const props = style({class: ${expression}});`,
    );
    assert.equal(evaluate(output.code!, { enabled: true }).props.className, expected, output.code);
  }
});
