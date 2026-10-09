import path from "node:path";
import { fileURLToPath } from "node:url";
import { runInNewContext } from "node:vm";

import { NativeDesignSystemLoader, NativeProjectSession } from "@stillwind/native";
import * as runtime from "stillwind/runtime/react";
import { expect, test } from "vite-plus/test";

import { compileModule, fileModuleDescriptor } from "../src/compilation/index.ts";
import { TypeScriptFactsService } from "../src/semantics/index.ts";

async function compile(files: Record<string, string>, entry = "entry.ts") {
  const root = path.dirname(fileURLToPath(import.meta.url));
  const loader = new NativeDesignSystemLoader();
  loader.provide(
    0,
    "design.css",
    "@utility red { color: red; } @utility blue { color: blue; } @utility green { color: green; } @stillwind-slot utilities;",
  );
  const session = new NativeProjectSession(loader, false);
  const facts = new TypeScriptFactsService({
    root,
    project: {
      kind: "isolated",
    },
  });
  try {
    const output = await compileModule(session, () => facts, files[entry], path.join(root, entry), {
      resolve: async (specifier, importer) => ({
        id: path.resolve(path.dirname(importer), `${specifier}.ts`),
      }),
      read: async (id) => {
        const source = files[path.basename(id)];
        expect(source).not.toBe(undefined);
        return source;
      },
      describe: (id) => ({
        ...fileModuleDescriptor(root, id),
        document: {
          ...fileModuleDescriptor(root, id).document,
          uri: `untitled:${encodeURIComponent(id)}`,
        },
      }),
      watch: () => {},
      warn: () => {},
    });
    expect(output).toBeTruthy();
    return output!;
  } finally {
    facts.close();
  }
}

test("nested transition branches resolve both imported producers", async () => {
  const output = await compile({
    "entry.ts": `
import { style } from "stillwind/react";
import { a, b } from "./barrel";

const enabled = Boolean(globalThis.enabled);
const outer = Boolean(globalThis.outer);
globalThis.props = style("green", outer ? (enabled && a.for("a")) : b.for("b"));
`,
    "barrel.ts": 'export { a, b } from "./transitions";\n',
    "transitions.ts": `
import { style } from "stillwind/react";

export const a = style.viewTransition({ old: "red" });
export const b = style.viewTransition({ old: "blue" });
`,
  });
  expect(output.code!).not.toMatch(/\.for\(/);
  const classes = [];
  for (const context of [
    { outer: true, enabled: false },
    { outer: true, enabled: true },
    { outer: false, enabled: false },
  ]) {
    const sandbox = { ...runtime, ...context, props: { className: "" } };
    runInNewContext(output.code!.replace(/^import\s+[^;]+;\s*/gm, ""), sandbox);
    classes.push(sandbox.props.className.split(" "));
  }
  expect(classes.map((value) => value.length)).toStrictEqual([1, 2, 2]);
  expect(classes[1].join()).not.toBe(classes[2].join());
});
test("enumerates factory and mutable conditional bases and preserves evaluation order", async () => {
  const output = await compile({
    "entry.ts": `
import { style } from "stillwind/react";

const a = style("red");
const b = style("green");
const events = [];
const select = () => {
  events.push("base");
  return flag ? a : b;
};
const props = style(
  { class: (events.push("external"), "outside") },
  select(),
  "blue",
  { "data-x": (events.push("attribute"), 1) }
);
globalThis.result = { props, events };`,
  });
  const context: Record<string, unknown> = { ...runtime, flag: false };
  runInNewContext(output.code!.replace(/^import\s+[^;]+;\s*/gm, ""), context);
  const result = context.result as {
    events: string[];
    props: Record<string, unknown>;
  };
  expect(Array.from(result.events)).toStrictEqual(["external", "base", "attribute"]);
  expect(result.props["data-x"]).toBe(1);
  expect(String(result.props.className).split(" ").length).toBe(3);
  expect(JSON.parse(output.compositionTopology).length).toBe(2);
  await expect(
    compile({
      "entry.ts": `
import { style} from "stillwind/react";

const a = style("red");
const b = style("green", { "data-x": 1 });
let selected: typeof a | typeof b = a;
if (flag) {
  selected = b;
}
const props = style(selected, "blue");
`,
    }),
  ).rejects.toThrow(/SW06005/);
});

test("rejects a CSS module cycle even when the style graph is acyclic", async () => {
  await expect(
    compile({
      "entry.ts": `
import { style } from "stillwind/react";
import { middle } from "./middle";

export const base = style("red");
export const derived = style(middle, "blue");
`,
      "middle.ts": `
import { style } from "stillwind/react";
import { base } from "./entry";

export const middle = style(base, "green");
`,
    }),
  ).rejects.toThrow(/SW06003/);
});

test("retains mutable candidates through exports and factories", async () => {
  await expect(
    compile({
      "entry.ts": `
import { style } from "stillwind/react";

const a = style("red");
const b = style("green", { "data-x": 1 } );
let selected: typeof a | typeof b = a;
if (flag) {
  selected = b;
}
function select() {
  return selected;
}
const props = style(select(), "blue");`,
    }),
  ).rejects.toThrow(/SW06005/);
  await expect(
    compile({
      "entry.ts": `
import { style } from "stillwind/react";
import { selected } from "./base";

const props = style(selected, "blue");
`,
      "base.ts": `
import { style} from "stillwind/react";

const a = style("red");
const b = style("green", { "data-x": 1 });

export let selected: typeof a | typeof b = a;

if (flag) {
  selected = b;
}
`,
    }),
  ).rejects.toThrow(/SW06005/);
});

test("parameterized factories preserve all base origins", async () => {
  await expect(
    compile({
      "entry.ts": `
import { style } from "stillwind/react";

const a = style("red");
const b = style("green", { "data-x": 1 });

function select(flag) {
  return flag ? a : b;
}

const props = style(select(flag), "blue");
`,
    }),
  ).rejects.toThrow(/SW06005/);
  const output = await compile({
    "entry.ts": `
import { style } from "stillwind/react";

const a = style("red");
const b = style("green");

function identity<T>(value: T) {
  return value;
}

const props = style(identity(flag ? a : b), "blue");`,
  });
  expect(output.composition.length).toBe(2);
  await expect(
    compile({
      "entry.ts": `
import { style } from "stillwind/react";
const a = style("red", { "data-x": 1 });
const b = style("green");
const choice : { value: typeof a | typeof b } = { value: a };
if (flag) {
  choice.value = b;
}
const props = style(choice.value, "blue");
`,
    }),
  ).rejects.toThrow(/SW06005/);
});

test("unrelated JavaScript cycles do not become CSS ordering errors", async () => {
  const output = await compile({
    "entry.ts": `
import { style } from "stillwind/react";
import { base } from "./base";

export function value() {
  return 1;
}
export const props = style(base, "blue");
`,
    "base.ts": `
import { style } from "stillwind/react";
import { value } from "./entry";

export function other() {
  return value();
}
export const base = style("red");`,
  });
  expect(output.code!).toMatch(/stillwindMerge/);
});

test("conditional utilities keep their branches alongside a runtime-selected base", async () => {
  const output = await compile({
    "entry.ts": `
import { style } from "stillwind/react";

const a = style("red");
const b = style("green");
const props = style(selectA ? a : b, enabled ? "blue" : "red");

globalThis.result = { a, b, props };
`,
  });
  expect(output.classes.length).toBe(4);
  for (const selectA of [false, true]) {
    for (const enabled of [false, true]) {
      const context: Record<string, unknown> = { ...runtime, selectA, enabled };
      runInNewContext(output.code!.replace(/^import\s+[^;]+;\s*/gm, ""), context);
      const result = context.result as Record<string, { className: string }>;
      const classes = result.props.className.split(" ");
      expect(classes.length).toBe(2);
      expect(classes[0]).toBe(result[selectA ? "a" : "b"].className);
      expect(classes[0]).not.toBe(classes[1]);
    }
  }
});

test("rejects unknown, props-bearing, nested, and incompatible-framework bases", async () => {
  for (const [body, error] of [
    ['const base = { className: "user" };\nconst props = style(base, "blue");', /SW12004/],
    [
      'const a = style("red");\nconst b = style("green", { "data-x": 1 });\nconst merged = style(a, b);\nconst props = style(merged, "blue");',
      /SW06005/,
    ],
    ['const props = style({ "data-x": style("red") });', /SW06001/],
  ] as const) {
    await expect(
      compile({
        "entry.ts": `import { style } from "stillwind/react";\n${body}\n`,
      }),
    ).rejects.toThrow(error);
  }
  await expect(
    compile({
      "entry.ts":
        'import { style } from "stillwind/react";\nimport { base } from "./base";\nconst props = style(base, \n"blue");\n',
      "base.ts": 'import { style } from "stillwind/solid";\nexport const base = style("red");\n',
    }),
  ).rejects.toThrow(/SW06006/);
});

test("composition follows imported and re-exported style producers", async () => {
  const root = path.dirname(fileURLToPath(import.meta.url));
  const files = new Map([
    [
      "entry.ts",
      'import { style } from "stillwind/react";\nimport { a, b } from "./barrel";\nexport const c = style(flag ? a : b, "blue");',
    ],
    ["barrel.ts", 'export { a, b } from "./base";\n'],
    [
      "base.ts",
      'import { style } from "stillwind/react";\nexport const a = style("red");\nexport const b = style("green");',
    ],
  ]);
  const loader = new NativeDesignSystemLoader();
  loader.provide(
    0,
    "design.css",
    "@utility red { color: red; }\n@utility blue { color: blue; }\n@utility green { color: green; }\n@stillwind-slot utilities;",
  );
  const session = new NativeProjectSession(loader, false);
  const facts = new TypeScriptFactsService({ root: root, project: { kind: "isolated" } });
  try {
    const compile = () =>
      compileModule(session, () => facts, files.get("entry.ts")!, path.join(root, "entry.ts"), {
        resolve: async (specifier, importer) => ({
          id: path.resolve(path.dirname(importer), `${specifier}.ts`),
        }),
        read: async (id) => files.get(path.basename(id))!,
        describe: (id) => ({
          ...fileModuleDescriptor(root, id),
          document: {
            ...fileModuleDescriptor(root, id).document,
            uri: `untitled:${encodeURIComponent(id)}`,
          },
        }),
        watch: () => {},
        warn: () => {},
      });
    const result = await compile();
    expect(result).toBeTruthy();
    expect(result!.code!).toMatch(
      /\(flag\) \? \(stillwindToFragment\(\(a\)\)\) : \(stillwindToFragment\(\(b\)\)\)/,
    );
    expect(result!.css).toMatch(/color:\s*blue/);
    expect(result!.css).not.toMatch(/color:\s*red|color:\s*green/);
    files.set(
      "base.ts",
      'import { style } from "stillwind/react";\nexport const a = style("red", { "data-x": 1 });\nexport const b = style("green");\n',
    );
    await expect(compile()).rejects.toThrow(/SW06005/);
  } finally {
    facts.close();
  }
});

test("JavaScript checker shadows preserve JSDoc identities and reject incompatible assignment", async () => {
  const output = await compile(
    {
      "entry.js": `
import { style } from "stillwind/react";

const a = style("red", /* retained */), b = style("green");

/**
 * @template T
 * @param {T} value
 * @returns {T}
 */
function identity(value) {
  return value;
}

const props = style(identity(Math.random() ? a : b), "blue");
`,
    },
    "entry.js",
  );
  expect(output.composition.length).toBe(2);
  expect(output.code!).not.toMatch(/__stillwindDeclarationIdentity/);
  await expect(
    compile(
      {
        "entry.js": `
import { style } from "stillwind/react";

const a = style("red");
const b = style("green");
let selected = a;
if (Math.random()) {
  selected = b;
}
const props = style(selected, "blue");
`,
      },
      "entry.js",
    ),
  ).rejects.toThrow(/SW12004/);
});

test("checker identity insertion accepts empty commented calls and trailing commas", async () => {
  const output = await compile({
    "entry.ts": `
import { style } from "stillwind/react";

const r = style.ref(/* marker */);
const a = style("red", /* trailing */);
const props = style(a, "blue", );
const target = style(r);`,
  });
  expect(output.composition.length).toBe(1);
});
