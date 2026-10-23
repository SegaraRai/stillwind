import path from "node:path";
import { fileURLToPath } from "node:url";
import { runInNewContext } from "node:vm";

import { TraceMap, originalPositionFor } from "@jridgewell/trace-mapping";
import { NativeDesignSystemLoader, NativeProjectSession } from "@stillwind/native";
import * as runtime from "stillwind/runtime/class-name-object";
import * as svelteRuntime from "stillwind/runtime/class-string";
import { expect, test } from "vite-plus/test";

import { compileModule, fileModuleDescriptor, moduleSourceMap } from "../src/compilation/index.ts";
import { TypeScriptFactsService } from "../src/semantics/index.ts";

async function compile(source: string, warnings: string[] = []) {
  const root = path.dirname(fileURLToPath(import.meta.url));
  const loader = new NativeDesignSystemLoader();
  loader.provide(
    0,
    "design.css",
    "@utility red { color: red } @utility blue { color: blue } @utility green { color: green } @stillwind-slot utilities;",
  );
  const session = new NativeProjectSession(loader, false);
  const facts = new TypeScriptFactsService({
    root,
    project: {
      kind: "isolated",
    },
  });
  try {
    const output = await compileModule(
      session,
      () => facts,
      `${source}\nglobalThis.result = { props, events };`,
      path.join(root, "entry.ts"),
      {
        resolve: async (specifier, importer) => ({
          id: path.resolve(path.dirname(importer), `${specifier}.ts`),
        }),
        read: async () => {
          expect.fail("unexpected dependency");
        },
        describe: (id) => ({
          ...fileModuleDescriptor(root, id),
          document: {
            ...fileModuleDescriptor(root, id).document,
            uri: `untitled:${encodeURIComponent(id)}`,
          },
        }),
        watch: () => {},
        warn: (message) => warnings.push(message),
      },
    );
    expect(output).toBeTruthy();
    return output!;
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

test("guarded view transitions preserve short-circuiting through type collection", async () => {
  const output = await compile(
    [
      'import {style} from "stillwind/react";',
      "const events = [];",
      "const enabled = Boolean(globalThis.enabled);",
      'const selected = style.viewTransition({old: "red", new: "blue"});',
      'const base = style.viewTransition({old: "green"});',
      'const key = () => { events.push("key"); return "guarded"; };',
      'const props = style("red", enabled && selected.for(key()), base.for("base"));',
    ].join("\n"),
  );
  const disabled = evaluate(output.code!, { enabled: false });
  const enabled = evaluate(output.code!, { enabled: true });
  expect(disabled.events.length).toBe(0);
  expect(enabled.events.join()).toBe("key");
  const baseClasses = String(disabled.props.className).split(" ");
  const enabledClasses = String(enabled.props.className).split(" ");
  expect(baseClasses.length).toBe(1);
  expect(enabledClasses.length).toBe(2);
  expect(enabledClasses).toContain(baseClasses[0]);
});

test("conditional transition trees select only the active class and key", async () => {
  const cases: [string, (enabled: boolean, outer: boolean) => string | null][] = [
    ["enabled ? a : b", () => "literal"],
    ['enabled ? a : b.for(key("b"))', (enabled) => (enabled ? "literal" : "b")],
    ['(enabled ? a.for("a") : b.for("b"))', () => "literal"],
    ['enabled ? a.for(key("a")) : b.for(key("b"))', (enabled) => (enabled ? "a" : "b")],
    ['enabled ? a.for("a") : b.for(key("b"))', (enabled) => (enabled ? "literal" : "b")],
    [
      'outer && (enabled ? a.for(key("a")) : b.for(key("b")))',
      (enabled, outer) => (outer ? (enabled ? "a" : "b") : null),
    ],
    [
      'outer ? (enabled ? a.for(key("a")) : b.for(key("b"))) : a.for(key("c"))',
      (enabled, outer) => (outer ? (enabled ? "a" : "b") : "c"),
    ],
    [
      'outer ? a.for(key("a")) : (enabled ? b.for(key("b")) : a.for(key("c")))',
      (enabled, outer) => (outer ? "a" : enabled ? "b" : "c"),
    ],
    ['(enabled ? a : b).for(key("selected"))', () => "selected"],
    ['(outer ? (enabled ? a : b) : a).for(key("selected"))', () => "selected"],
    [
      '(outer ? enabled : !enabled) && a.for(key("a"))',
      (enabled, outer) => ((outer ? enabled : !enabled) ? "a" : null),
    ],
    [
      'outer ? (enabled && a.for(key("a"))) : b.for(key("b"))',
      (enabled, outer) => (outer ? (enabled ? "a" : null) : "b"),
    ],
    ...["false", "null", "undefined"].flatMap(
      (empty) =>
        [
          [`enabled ? (a.for(key("a"))) : ${empty}`, (enabled: boolean) => (enabled ? "a" : null)],
          [`enabled ? ${empty} : (b.for(key("b")))`, (enabled: boolean) => (enabled ? null : "b")],
        ] as [string, (enabled: boolean, outer: boolean) => string | null][],
    ),
  ];
  for (const [expression, selected] of cases) {
    const warnings: string[] = [];
    const output = await compile(
      [
        'import { style } from "stillwind/react";',
        "const events = [];",
        "const enabled = Boolean(globalThis.enabled), outer = Boolean(globalThis.outer);",
        'const a = style.viewTransition({ old: "red" });',
        'const b = style.viewTransition({ old: "blue" });',
        "const key = value => { events.push(value); return value; };",
        `const props = style("green", ${expression});`,
      ].join("\n"),
      warnings,
    );
    expect(
      !warnings.some((message) => message.includes("duplicate-view-transition-name")),
      warnings.join("\n"),
    ).toBeTruthy();
    expect(output.code!, expression).not.toMatch(/\.for\(/);
    for (const enabled of [false, true]) {
      for (const outer of [false, true]) {
        const expected = selected(enabled, outer);
        const { props, events } = evaluate(output.code!, { enabled, outer });
        expect(String(props.className).split(" ").length, expression).toBe(expected ? 2 : 1);
        expect(events.join(), expression).toBe(expected === "literal" ? "" : (expected ?? ""));
        if (expected && expected !== "literal") {
          expect(
            String((props.style as Record<string, unknown>)["view-transition-name"]).endsWith(
              "-" + expected,
            ),
            expression,
          ).toBeTruthy();
        }
      }
    }
  }
});

test("conditional ref and keyframes receivers retain finite candidates", async () => {
  for (const [declarations, expression] of [
    ['const a = style.ref("a"), b = style.ref("b");', "enabled ? a : b"],
    [
      'const a = style.keyframes({ from: "red" }), b = style.keyframes({ from: "blue" });',
      '(enabled ? a : b)("")',
    ],
  ]) {
    const output = await compile(
      [
        'import { style } from "stillwind/react";',
        "const events = [];",
        "const enabled = Boolean(globalThis.enabled);",
        declarations,
        `const props = style("green", ${expression});`,
      ].join("\n"),
    );
    const enabled = evaluate(output.code!, { enabled: true }).props;
    const disabled = evaluate(output.code!, { enabled: false }).props;
    expect(enabled.className, expression).not.toBe(disabled.className);
    expect(String(enabled.className)).not.toMatch(/undefined/);
    expect(String(disabled.className)).not.toMatch(/undefined/);
  }
});

test("a separate name still conflicts with a conditional transition name", async () => {
  const warnings: string[] = [];
  await compile(
    [
      'import {style} from "stillwind/react";',
      "const events = [];",
      "const enabled = Boolean(globalThis.enabled);",
      'const a = style.viewTransition({ old: "red" }), b = style.viewTransition({ old: "blue" });',
      'const props = style(a.for("base"), enabled ? a.for("a") : b.for("b"));',
    ].join("\n"),
    warnings,
  );
  expect(
    warnings.filter((message) => message.includes("duplicate-view-transition-name")).length,
  ).toBe(1);
  expect(
    warnings.some((message) => message.includes("2 view-transition-name sources")),
  ).toBeTruthy();
});

test("inactive transition payloads preserve preceding styles in both formats", async () => {
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
            `import { style } from "stillwind/${framework}";`,
            "const events = [];",
            "const enabled = Boolean(globalThis.enabled), outer = Boolean(globalThis.outer);",
            'const a = style.viewTransition({ old: "red" }), b = style.viewTransition({ old: "blue" });',
            'const baseKey = "base", optionalKey = "optional";',
            `const props = style("green", ${withBase ? "a.for(baseKey)," : ""} ${expression});`,
          ].join("\n"),
        );
        for (const enabled of [false, true]) {
          for (const outer of [false, true]) {
            const context = framework === "svelte" ? svelteRuntime : runtime;
            const { props } = evaluate(output.code!, {
              ...context,
              enabled,
              outer,
            });
            const suffix = active(enabled, outer) ? "optional" : withBase ? "base" : null;
            if (suffix === null) {
              expect(Object.hasOwn(props, "style"), expression).toBe(false);
            } else {
              const style =
                framework === "svelte"
                  ? props.style
                  : (props.style as Record<string, unknown>)?.["view-transition-name"];
              if (!(typeof style === "string" && style.endsWith("-" + suffix))) {
                expect.fail(
                  JSON.stringify({
                    framework,
                    expression,
                    enabled,
                    outer,
                    props,
                  }),
                );
              }
            }
          }
        }
      }
    }
  }
});

test("common input branches select transitions, custom properties, refs, and composed styles", async () => {
  for (const framework of ["react", "svelte"]) {
    const output = await compile(
      [
        `import {style} from "stillwind/${framework}";`,
        "const events = [];",
        "const enabled = Boolean(globalThis.enabled), outer = Boolean(globalThis.outer);",
        'const transition = style.viewTransition({ old: "red" }); const key = "selected";',
        'const ref = style.ref("target"); const base = style("blue");',
        'const props = style("green", enabled ? transition.for(key) : { "--progress": "1" }, outer && (enabled ? ref : base));',
      ].join("\n"),
    );
    for (const enabled of [false, true]) {
      for (const outer of [false, true]) {
        const { props } = evaluate(output.code!, {
          ...(framework === "svelte" ? svelteRuntime : runtime),
          enabled,
          outer,
        });
        const style = props.style;
        if (enabled) {
          const name =
            framework === "svelte"
              ? style
              : (style as Record<string, unknown>)["view-transition-name"];
          expect(String(name).endsWith("-selected")).toBeTruthy();
        } else if (framework === "svelte") {
          expect(style).toBe("--progress:1");
        } else {
          expect((style as Record<string, unknown>)["--progress"]).toBe("1");
        }
        const classes = String(props[framework === "svelte" ? "class" : "className"]).split(" ");
        expect(classes.length).toBe(1 + Number(enabled) + Number(outer));
      }
    }
  }
});

test("utility and transition names share branch exclusivity for linting", async () => {
  const warnings: string[] = [];
  await compile(
    [
      'import {style} from "stillwind/react";',
      "const events = [];",
      "const enabled = Boolean(globalThis.enabled);",
      'const a = style.viewTransition({ old: "red" });',
      'const props = style(a, enabled ? "[view-transition-name:first]" : "[view-transition-name:second]");',
    ].join("\n"),
    warnings,
  );
  expect(
    !warnings.some((message) => message.includes("duplicate-view-transition-name")),
    warnings.join("\n"),
  ).toBeTruthy();
});

test("pure conditional calls select one class branch", async () => {
  const output = await compile(
    [
      'import { style } from "stillwind/react";',
      "const first = () => enabled;",
      "const events = [];",
      'const props = style("red", first() ? "blue" : "green");',
    ].join("\n"),
  );
  const yes = evaluate(output.code!, { enabled: true }).props;
  const no = evaluate(output.code!, { enabled: false }).props;
  expect(String(yes.className).split(" ").length).toBe(2);
  expect(String(no.className).split(" ").length).toBe(2);
  expect(yes.className).not.toBe(no.className);
});

test("short-circuiting leaves an unselected nested value unread", async () => {
  const output = await compile(
    [
      'import { style } from "stillwind/react";',
      "const events = [];",
      'const props = style("red", enabled && (second() ? "blue" : "green"));',
    ].join("\n"),
  );
  const disabled = evaluate(output.code!, { enabled: false });
  expect(String(disabled.props.className).split(" ").length).toBe(1);
  const enabled = evaluate(output.code!, { enabled: true, second: () => true });
  expect(String(enabled.props.className).split(" ").length).toBe(2);
});

test("pure custom-property and guard calls materialize directly", async () => {
  const output = await compile(
    [
      'import { style } from "stillwind/react";',
      "const events = [];",
      "const read = (value) => value;",
      'const props = style({ "--busy": read(true) }, read(enabled) ? "blue" : "green");',
    ].join("\n"),
  );
  expect(output.code!).not.toMatch(/stillwindMerge/);
  const { props } = evaluate(output.code!, { enabled: true });
  expect(Object.keys(props)).toStrictEqual(["className", "style"]);
  expect((props.style as Record<string, string>)["--busy"]).toBe(" ");
});

test("direct props and fragment materialization produce the same classes", async () => {
  const direct = await compile(
    [
      'import { style } from "stillwind/react";',
      "const events = [];",
      'const props = style("red", enabled && (nested ? "blue" : "green"));',
    ].join("\n"),
  );
  const fragment = await compile(
    [
      'import { style } from "stillwind/react";',
      'const base = style("red");',
      "const events = [];",
      'const props = style(base, enabled && (nested ? "blue" : "green"));',
    ].join("\n"),
  );
  expect(direct.code!).not.toMatch(/stillwindMerge/);
  expect(fragment.code!).toMatch(/stillwindMerge\(stillwindToFragment\(\(base\)\)/);
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
    expect(secondClasses.length, JSON.stringify(context)).toBe(firstClasses.length);
    expect(secondClasses.length > 1, JSON.stringify(context)).toBe(firstClasses.length > 1);
    // The unconditional base unit stays first and stable in both representations.
    bases.direct.add(firstClasses[0]);
    bases.fragment.add(secondClasses[0]);
  }
  expect(bases.direct.size).toBe(1);
  expect(bases.fragment.size).toBe(1);
});

test("fragments stay inside the call expression", async () => {
  const output = await compile(
    [
      'import {style} from "stillwind/react";',
      "const events = [];",
      'const props = style({ "--busy": enabled }, enabled ? "blue" : "green", nested && "red");',
    ].join("\n"),
  );
  expect(output.code!).not.toMatch(/Symbol\(/);
  const nonImport = output
    .code!.split("\n")
    .filter((line) => !line.startsWith("import "))
    .join("\n");
  const keyUses = nonImport.match(/stillwindClassKey/g)?.length ?? 0;
  expect(nonImport.match(/\[stillwindClassKey\]/g)?.length ?? 0).toBe(keyUses);
  const { props } = evaluate(output.code!, { enabled: true, nested: false });
  expect((props.style as Record<string, string>)["--busy"]).toBe(" ");
  expect(String(props.className).split(" ").length).toBe(1);
});

test("custom-property folding materializes object and string framework props", async () => {
  for (const [framework, helpers] of [
    ["react", runtime],
    ["svelte", svelteRuntime],
  ] as const) {
    const output = await compile(
      [
        `import { style } from "stillwind/${framework}";`,
        "const events = [];",
        'const props = style("red", { "--progress": (events.push("value"), progress) });',
      ].join("\n"),
    );
    expect(output.code!).not.toMatch(
      /stillwindMerge|stillwindCustomPropertiesKey|stillwindClassKey/,
    );
    for (const [progress, expected] of [
      [0, "0"],
      [true, " "],
      [false, "initial"],
      ["calc(1 + 2)", "calc(1 + 2)"],
    ] as const) {
      const { props, events } = evaluate(output.code!, {
        ...helpers,
        progress,
      });
      expect(Array.from(events)).toStrictEqual(["value"]);
      expect(Object.keys(props)).toStrictEqual([
        framework === "react" ? "className" : "class",
        "style",
      ]);
      if (framework === "svelte") {
        expect(props.style).toBe(`--progress:${expected}`);
      } else {
        expect((props.style as Record<string, unknown>)["--progress"]).toBe(expected);
      }
    }
  }
});

test("interleaved pure calls fold into one property record", async () => {
  const output = await compile(
    [
      'import { style } from "stillwind/react";',
      "const events = [];",
      "const read = (value) => value;",
      'const props = style({ "--x": read(1) }, { "--value": read(2) }, { "--y": read(3) });',
    ].join("\n"),
  );
  expect(output.code!).not.toMatch(/stillwindMerge/);
  const { props } = evaluate(output.code!, {});
  expect(Object.entries(props.style as object)).toStrictEqual([
    ["--x", "1"],
    ["--value", "2"],
    ["--y", "3"],
  ]);
});

test("empty branches omit calls while nonempty branches keep guards", async () => {
  for (const branch of [
    "check() ? [] : []",
    "check() && []",
    "check() ? [[], { '--x': 1 }] : []",
  ]) {
    const output = await compile(
      [
        'import { style } from "stillwind/react";',
        "const events = [];",
        `const props = style(${branch});`,
      ].join("\n"),
    );
    for (const enabled of [false, true]) {
      const { props } = evaluate(
        output.code!,
        branch.includes("--x") ? { check: () => enabled } : {},
      );
      expect(Object.hasOwn(props, "className")).toBe(false);
      expect(Object.hasOwn(props, "style")).toBe(enabled && branch.includes("--x"));
    }
  }
});

test("pure folding and empty-branch removal preserve materialized values", async () => {
  for (const [framework, helpers] of [
    ["react", runtime],
    ["svelte", svelteRuntime],
  ] as const) {
    const output = await compile(
      [
        `import { style } from "stillwind/${framework}";`,
        "const events = []; const read = (value) => value;",
        'const props = style({ "--x": read(x) }, { "--value": x }, { "--y": x + 1 }, enabled && [], "red");',
      ].join("\n"),
    );
    expect(output.code!).not.toMatch(/stillwindMerge|enabled/);
    const { props } = evaluate(output.code!, { ...helpers, x: 3 });
    expect(Object.keys(props)).toStrictEqual([
      framework === "react" ? "className" : "class",
      "style",
    ]);
    if (framework === "svelte") {
      expect(props.style).toBe("--x:3;--value:3;--y:4");
    } else {
      expect(Object.entries(props.style as object)).toStrictEqual([
        ["--x", "3"],
        ["--value", "3"],
        ["--y", "4"],
      ]);
    }
  }
});

test("macro mutations and execution boundaries preserve output and denied lint reports", async () => {
  for (const [expression, code] of [
    ["value++", "mutating-macro-argument"],
    ["(import('module'), 1)", "suspending-macro-argument"],
  ]) {
    const warnings: string[] = [];
    const output = await compile(
      [
        'import { style } from "stillwind/react"; ',
        "const events = []; let value = 1;",
        `const props = style({ "--x": ${expression} });`,
      ].join("\n"),
      warnings,
    );
    expect(output.reports).toContainEqual(expect.objectContaining({ code, severity: "error" }));
    expect(warnings[0]).toContain(code);
  }
});

test("folded expressions map to distinct original occurrences through native output", async () => {
  const position = (text: string, offset: number) => ({
    line: text.slice(0, offset).split("\n").length,
    column: offset - text.lastIndexOf("\n", offset - 1) - 1,
  });
  for (const framework of ["react", "preact", "solid", "vue", "svelte", "astro"]) {
    for (const call of [
      'style({ "--x": same },\r\n { class: other },\r\n { "--y": same }, "red")',
      'style({ "--x": other }, { class: [same, [,, "😀日本", same]] }, "red")',
      'style(check() ? { "--x": first() } : { "--y": last() })',
      'style("red", enabled && "blue", { class: extra })',
      'style({ "--x": `😀${value}\n日本語` })',
      'style(flag && [], { "--x": remaining })',
    ]) {
      const source = `import {style} from "stillwind/${framework}";\r\n// 😀 日本語\r\nconst events = [];\r\nconst props = ${call};`;
      const fullSource = `${source}\nglobalThis.result = { props, events };`;
      const output = await compile(source);
      const map = new TraceMap(
        moduleSourceMap(fullSource, output.code!, "/entry.ts", output.mappings, [], ""),
      );
      const expressions = output.mappings.flatMap((mapping) => mapping.expressions);
      expect(expressions.length).toBeGreaterThan(0);
      for (const expression of expressions) {
        expect(expression.exact).toBe(true);
        const originalText = fullSource.slice(expression.originalStart, expression.originalEnd);
        expect(output.code!.slice(expression.generatedStart, expression.generatedEnd)).toBe(
          originalText,
        );
        // Every UTF-16 code unit, including surrogate interiors and line breaks.
        for (let offset = 0; offset < originalText.length; offset++) {
          const actual = originalPositionFor(
            map,
            position(output.code!, expression.generatedStart + offset),
          );
          const expected = position(fullSource, expression.originalStart + offset);
          expect(
            { line: actual.line, column: actual.column },
            `${framework}: ${call}: ${offset}`,
          ).toStrictEqual(expected);
        }
      }
      if (call.includes("same")) {
        const occurrences = expressions.filter(
          (expression) =>
            fullSource.slice(expression.originalStart, expression.originalEnd) === "same",
        );
        expect(occurrences.length).toBe(2);
        expect(occurrences[0].originalStart).not.toBe(occurrences[1].originalStart);
        expect(
          expressions.some(
            (expression, index) =>
              index > 0 && expression.originalStart < expressions[index - 1].originalStart,
          ),
        ).toBeTruthy();
      }
      if (call.includes("flag")) {
        expect(
          expressions.every(
            (expression) =>
              fullSource.slice(expression.originalStart, expression.originalEnd) !== "flag",
          ),
        ).toBeTruthy();
      }
    }
  }
});

test("external class arrays flatten while spread keeps iterable semantics", async () => {
  for (const [expression, expected] of [
    ['["first", [,, "日本😀", ["last"]]]', "first 日本😀 last"],
    ['[..."ab"]', "a b"],
    ['[[...new Set(["a", "b"])], "c"]', "a b c"],
    ['[enabled ? ["a", "b"] : [], "c"]', "a b c"],
    ['[("ignored", "kept"), "last"]', "kept last"],
    ["[[], [,,]]", undefined],
  ] as const) {
    const output = await compile(
      `import {style} from "stillwind/react"; const events = []; const props = style({class: ${expression}});`,
    );
    expect(evaluate(output.code!, { enabled: true }).props.className, output.code!).toBe(expected);
  }
});

test("transition class branches and complete style payloads avoid fragment merge", async () => {
  for (const framework of ["react", "svelte"]) {
    for (const input of ["enabled && a", "enabled ? a : b", "a.for(key)"]) {
      const output = await compile(
        [
          `import {style} from "stillwind/${framework}"; const events = [];`,
          "const enabled = Boolean(globalThis.enabled), key = globalThis.key;",
          'const a = style.viewTransition({ old: "red" }), b = style.viewTransition({ old: "blue" });',
          `const props = style("green", ${input});`,
        ].join("\n"),
      );
      expect(output.code!).not.toMatch(/stillwindMerge|stillwindClassKey|stillwindStyleKey/);
      for (const enabled of [false, true]) {
        const { props } = evaluate(output.code!, {
          ...(framework === "svelte" ? svelteRuntime : runtime),
          enabled,
          key: "selected",
        });
        const classes = String(props[framework === "svelte" ? "class" : "className"]).split(" ");
        expect(classes.length).toBe(
          input === "a.for(key)" || (input === "enabled && a" && !enabled) ? 1 : 2,
        );
        if (input === "a.for(key)") {
          expect(JSON.stringify(props.style)).toContain("-selected");
        }
      }
    }
  }
});

test("style strings fold static segments and convert only dynamic values", async () => {
  for (const framework of ["svelte", "astro"]) {
    const output = await compile(
      [
        `import { style } from "stillwind/${framework}";`,
        "const events = [];",
        "const amount = globalThis.amount;",
        'const props = style({ "--static": "foo", "--dynamic": amount, "--tail": "bar", "--end": false });',
      ].join("\n"),
    );
    expect(output.code!).toMatch(
      /style:"--static:foo;--dynamic:"\+stillwindCssValue\(amount\)\+";--tail:bar;--end:initial"/,
    );
    expect((output.code!.match(/stillwindCssValue\(/g) ?? []).length).toBe(1);
    for (const amount of [3, false, "a;b", new URL("https://example.test/")]) {
      const { props } = evaluate(output.code!, { ...svelteRuntime, amount });
      expect(props.style).toBe(
        `--static:foo;--dynamic:${svelteRuntime.stillwindCssValue(amount)};--tail:bar;--end:initial`,
      );
    }
    const literal = await compile(
      [
        `import { style } from "stillwind/${framework}";`,
        "const events = [];",
        'const props = style({ "--foo": "bar", "--bar": `baz`, "--tiny": 1e-7, "--large": 1e21, "--negative": -2, "--flag": true });',
      ].join("\n"),
    );
    expect(literal.code!).toMatch(
      /style:"--foo:bar;--bar:baz;--tiny:1e-7;--large:1e\+21;--negative:-2;--flag: "/,
    );
    expect(literal.code!).not.toMatch(/stillwindCssValue|stillwindMerge/);
  }
});

test("class-only conditional props use the framework field even when empty", async () => {
  for (const framework of ["react", "svelte"]) {
    const output = await compile(
      [
        `import { style } from "stillwind/${framework}";`,
        "const events = [];",
        "const enabled = Boolean(globalThis.enabled);",
        'const props = style(enabled && "red");',
      ].join("\n"),
    );
    expect(output.code!).not.toMatch(/stillwindClassProps|stillwindMerge/);
    const { props } = evaluate(output.code!, {
      ...svelteRuntime,
      enabled: false,
    });
    const key = framework === "react" ? "className" : "class";
    expect(Object.hasOwn(props, key)).toBeTruthy();
    expect(props[key]).toBe("");
  }
});
