import assert from "node:assert/strict";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";
import { runInNewContext } from "node:vm";

import { TraceMap, originalPositionFor } from "@jridgewell/trace-mapping";
import { NativeDesignSystemLoader, NativeProjectSession } from "@stillwind/native";

import * as runtime from "../../core/src/runtime/react.ts";
import * as svelteRuntime from "../../core/src/runtime/svelte.ts";
import { compileModule, fileModuleDescriptor } from "../src/compiler.ts";
import { moduleSourceMap } from "../src/source-map.ts";
import { TypeScriptFactsService } from "../src/type-facts.ts";

async function compile(source: string, warnings: string[] = []) {
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
        describe: (id) => ({
          ...fileModuleDescriptor(root, id),
          uri: `untitled:${encodeURIComponent(id)}`,
        }),
        watch: () => {},
        warn: (message) => warnings.push(message),
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

void test("conditional transition trees select only the active class and key", async () => {
  const cases: Array<[string, (enabled: boolean, outer: boolean) => string | null]> = [
    ["enabled ? a : b", () => "literal"],
    ["enabled ? a : b.for(key('b'))", (enabled) => (enabled ? "literal" : "b")],
    ["(enabled ? a.for('a') : b.for('b'))", () => "literal"],
    ["enabled ? a.for(key('a')) : b.for(key('b'))", (enabled) => (enabled ? "a" : "b")],
    ["enabled ? a.for('a') : b.for(key('b'))", (enabled) => (enabled ? "literal" : "b")],
    [
      "outer && (enabled ? a.for(key('a')) : b.for(key('b')))",
      (enabled, outer) => (outer ? (enabled ? "a" : "b") : null),
    ],
    [
      "outer ? (enabled ? a.for(key('a')) : b.for(key('b'))) : a.for(key('c'))",
      (enabled, outer) => (outer ? (enabled ? "a" : "b") : "c"),
    ],
    [
      "outer ? a.for(key('a')) : (enabled ? b.for(key('b')) : a.for(key('c')))",
      (enabled, outer) => (outer ? "a" : enabled ? "b" : "c"),
    ],
    ["(enabled ? a : b).for(key('selected'))", () => "selected"],
    ["(outer ? (enabled ? a : b) : a).for(key('selected'))", () => "selected"],
    [
      "(outer ? enabled : !enabled) && a.for(key('a'))",
      (enabled, outer) => ((outer ? enabled : !enabled) ? "a" : null),
    ],
    [
      "outer ? (enabled && a.for(key('a'))) : b.for(key('b'))",
      (enabled, outer) => (outer ? (enabled ? "a" : null) : "b"),
    ],
    ...["false", "null", "undefined"].flatMap(
      (empty) =>
        [
          [`enabled ? (a.for(key('a'))) : ${empty}`, (enabled: boolean) => (enabled ? "a" : null)],
          [`enabled ? ${empty} : (b.for(key('b')))`, (enabled: boolean) => (enabled ? null : "b")],
        ] as Array<[string, (enabled: boolean, outer: boolean) => string | null]>,
    ),
  ];
  for (const [expression, selected] of cases) {
    const warnings: string[] = [];
    const output = await compile(
      [
        "import {style} from 'stillwind/react'; const events = [];",
        "const enabled = Boolean(globalThis.enabled), outer = Boolean(globalThis.outer);",
        "const a = style.viewTransition({old: 'red'});",
        "const b = style.viewTransition({old: 'blue'});",
        "const key = value => { events.push(value); return value; };",
        `const props = style('green', ${expression});`,
      ].join("\n"),
      warnings,
    );
    assert.ok(
      !warnings.some((message) => message.includes("single-view-transition-name")),
      warnings.join("\n"),
    );
    assert.doesNotMatch(output.code!, /\.for\(/, expression);
    for (const enabled of [false, true]) {
      for (const outer of [false, true]) {
        const expected = selected(enabled, outer);
        const { props, events } = evaluate(output.code!, { enabled, outer });
        assert.equal(String(props.className).split(" ").length, expected ? 2 : 1, expression);
        assert.equal(events.join(), expected === "literal" ? "" : (expected ?? ""), expression);
        if (expected && expected !== "literal") {
          assert.ok(
            String((props.style as Record<string, unknown>)["view-transition-name"]).endsWith(
              "-" + expected,
            ),
            expression,
          );
        }
      }
    }
  }
});

void test("conditional ref and keyframes receivers retain finite candidates", async () => {
  for (const [declarations, expression] of [
    ["const a = style.ref('a'), b = style.ref('b');", "enabled ? a : b"],
    [
      "const a = style.keyframes({from:'red'}), b = style.keyframes({from:'blue'});",
      "(enabled ? a : b)('')",
    ],
  ]) {
    const output = await compile(
      [
        "import {style} from 'stillwind/react'; const events = [];",
        "const enabled = Boolean(globalThis.enabled);",
        declarations,
        `const props = style('green', ${expression});`,
      ].join("\n"),
    );
    const enabled = evaluate(output.code!, { enabled: true }).props;
    const disabled = evaluate(output.code!, { enabled: false }).props;
    assert.notEqual(enabled.className, disabled.className, expression);
    assert.doesNotMatch(String(enabled.className), /undefined/);
    assert.doesNotMatch(String(disabled.className), /undefined/);
  }
});

void test("a separate name still conflicts with a conditional transition name", async () => {
  const warnings: string[] = [];
  await compile(
    [
      "import {style} from 'stillwind/react'; const events = [];",
      "const enabled = Boolean(globalThis.enabled);",
      "const a = style.viewTransition({old: 'red'}), b = style.viewTransition({old: 'blue'});",
      "const props = style(a.for('base'), enabled ? a.for('a') : b.for('b'));",
    ].join("\n"),
    warnings,
  );
  assert.equal(
    warnings.filter((message) => message.includes("single-view-transition-name")).length,
    1,
  );
  assert.ok(warnings.some((message) => message.includes("2 view-transition-name sources")));
});

void test("inactive transition payloads preserve preceding styles in both formats", async () => {
  for (const framework of ["react", "svelte"]) {
    for (const withBase of [false, true]) {
      for (const [expression, active] of [
        ["enabled && b.for(optionalKey)", (enabled: boolean, _outer: boolean) => enabled],
        ["enabled ? null : b.for(optionalKey)", (enabled: boolean, _outer: boolean) => !enabled],
        [
          "outer && (enabled ? b.for(optionalKey) : null)",
          (enabled: boolean, outer: boolean) => outer && enabled,
        ],
      ] as const) {
        const output = await compile(
          [
            `import {style} from 'stillwind/${framework}'; const events = [];`,
            "const enabled = Boolean(globalThis.enabled), outer = Boolean(globalThis.outer);",
            "const a = style.viewTransition({old:'red'}), b = style.viewTransition({old:'blue'});",
            "const baseKey = 'base', optionalKey = 'optional';",
            `const props = style('green', ${withBase ? "a.for(baseKey)," : ""} ${expression});`,
          ].join("\n"),
        );
        for (const enabled of [false, true]) {
          for (const outer of [false, true]) {
            const context = framework === "svelte" ? svelteRuntime : runtime;
            const { props } = evaluate(output.code!, { ...context, enabled, outer });
            const suffix = active(enabled, outer) ? "optional" : withBase ? "base" : null;
            if (suffix === null) {
              assert.equal(Object.hasOwn(props, "style"), false, expression);
            } else {
              const style =
                framework === "svelte"
                  ? props.style
                  : (props.style as Record<string, unknown>)?.["view-transition-name"];
              assert.ok(
                typeof style === "string" && style.endsWith("-" + suffix),
                JSON.stringify({ framework, expression, enabled, outer, props }),
              );
            }
          }
        }
      }
    }
  }
});

void test("common input branches select transitions, props, refs, and composed styles", async () => {
  for (const framework of ["react", "svelte"]) {
    const output = await compile(
      [
        `import {style} from 'stillwind/${framework}'; const events = [];`,
        "const enabled = Boolean(globalThis.enabled), outer = Boolean(globalThis.outer);",
        "const transition = style.viewTransition({old:'red'}); const key = 'selected';",
        "const ref = style.ref('target'); const base = style('blue');",
        "const props = style('green', enabled ? transition.for(key) : {'data-state':'off','--progress':'1'}, outer && (enabled ? ref : base));",
      ].join("\n"),
    );
    for (const enabled of [false, true]) {
      for (const outer of [false, true]) {
        const { props } = evaluate(output.code!, {
          ...(framework === "svelte" ? svelteRuntime : runtime),
          enabled,
          outer,
        });
        assert.equal(props["data-state"], enabled ? undefined : "off");
        const style = props.style;
        if (enabled) {
          const name =
            framework === "svelte"
              ? style
              : (style as Record<string, unknown>)["view-transition-name"];
          assert.ok(String(name).endsWith("-selected"));
        } else if (framework === "svelte") {
          assert.equal(style, "--progress:1");
        } else {
          assert.equal((style as Record<string, unknown>)["--progress"], "1");
        }
        const classes = String(props[framework === "svelte" ? "class" : "className"]).split(" ");
        assert.equal(classes.length, 1 + Number(enabled) + Number(outer));
      }
    }
  }
});

void test("utility and transition names share branch exclusivity for linting", async () => {
  const warnings: string[] = [];
  await compile(
    [
      "import {style} from 'stillwind/react'; const events = [];",
      "const enabled = Boolean(globalThis.enabled);",
      "const a = style.viewTransition({old:'red'});",
      "const props = style(a, enabled ? '[view-transition-name:first]' : '[view-transition-name:second]');",
    ].join("\n"),
    warnings,
  );
  assert.ok(
    !warnings.some((message) => message.includes("single-view-transition-name")),
    warnings.join("\n"),
  );
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
        const originalText = fullSource.slice(expression.originalStart, expression.originalEnd);
        assert.equal(
          output.code!.slice(expression.generatedStart, expression.generatedEnd),
          originalText,
        );
        // Every UTF-16 code unit, including surrogate interiors and line breaks.
        for (let offset = 0; offset < originalText.length; offset++) {
          const actual = originalPositionFor(
            map,
            position(output.code!, expression.generatedStart + offset),
          );
          const expected = position(fullSource, expression.originalStart + offset);
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
            fullSource.slice(expression.originalStart, expression.originalEnd) === "same",
        );
        assert.equal(occurrences.length, 2);
        assert.notEqual(occurrences[0].originalStart, occurrences[1].originalStart);
        assert.ok(
          expressions.some(
            (expression, index) =>
              index > 0 && expression.originalStart < expressions[index - 1].originalStart,
          ),
        );
      }
      if (call.includes("flag")) {
        assert.ok(
          expressions.every(
            (expression) =>
              fullSource.slice(expression.originalStart, expression.originalEnd) !== "flag",
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

void test("transition class branches and complete style payloads avoid fragment merge", async () => {
  for (const framework of ["react", "svelte"]) {
    for (const input of ["enabled && a", "enabled ? a : b", "a.for(key)"]) {
      const output = await compile(
        [
          `import {style} from 'stillwind/${framework}'; const events = [];`,
          "const enabled = Boolean(globalThis.enabled), key = globalThis.key;",
          "const a = style.viewTransition({old:'red'}), b = style.viewTransition({old:'blue'});",
          `const props = style('green', ${input});`,
        ].join("\n"),
      );
      assert.doesNotMatch(output.code!, /stillwindMerge|stillwindClassKey|stillwindStyleKey/);
      for (const enabled of [false, true]) {
        const { props } = evaluate(output.code!, {
          ...(framework === "svelte" ? svelteRuntime : runtime),
          enabled,
          key: "selected",
        });
        const classes = String(props[framework === "svelte" ? "class" : "className"]).split(" ");
        assert.equal(
          classes.length,
          input === "a.for(key)" || (input === "enabled && a" && !enabled) ? 1 : 2,
        );
        if (input === "a.for(key)") {
          assert.ok(JSON.stringify(props.style).includes("-selected"));
        }
      }
    }
  }
});

void test("style strings fold static segments and convert only dynamic values", async () => {
  for (const framework of ["svelte", "astro"]) {
    const output = await compile(
      [
        `import {style} from 'stillwind/${framework}'; const events = [];`,
        "const amount = globalThis.amount;",
        "const props = style({'--static':'foo','--dynamic':amount,'--tail':'bar','--end':false});",
      ].join("\n"),
    );
    assert.match(
      output.code!,
      /style:"--static:foo;--dynamic:"\+stillwindCssValue\(amount\)\+";--tail:bar;--end:initial"/,
    );
    assert.equal((output.code!.match(/stillwindCssValue\(/g) ?? []).length, 1);
    for (const amount of [3, false, "a;b", new URL("https://example.test/")]) {
      const { props } = evaluate(output.code!, { ...svelteRuntime, amount });
      assert.equal(
        props.style,
        `--static:foo;--dynamic:${svelteRuntime.stillwindCssValue(amount)};--tail:bar;--end:initial`,
      );
    }
    const literal = await compile(
      [
        `import {style} from 'stillwind/${framework}'; const events = [];`,
        "const props = style({'--foo':'bar','--bar':`baz`,'--tiny':1e-7,'--large':1e21,'--negative':-2,'--flag':true});",
      ].join("\n"),
    );
    assert.match(
      literal.code!,
      /style:"--foo:bar;--bar:baz;--tiny:1e-7;--large:1e\+21;--negative:-2;--flag: "/,
    );
    assert.doesNotMatch(literal.code!, /stillwindCssValue|stillwindMerge/);
  }
});

void test("class-only conditional props use the framework field even when empty", async () => {
  for (const framework of ["react", "svelte"]) {
    const output = await compile(
      [
        `import {style} from 'stillwind/${framework}'; const events = [];`,
        "const enabled = Boolean(globalThis.enabled);",
        "const props = style(enabled && 'red');",
      ].join("\n"),
    );
    assert.doesNotMatch(output.code!, /stillwindClassProps|stillwindMerge/);
    const { props } = evaluate(output.code!, { ...svelteRuntime, enabled: false });
    const key = framework === "react" ? "className" : "class";
    assert.ok(Object.hasOwn(props, key));
    assert.equal(props[key], "");
  }
});
