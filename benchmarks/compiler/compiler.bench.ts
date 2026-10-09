import { createHash } from "node:crypto";

import { resolveFixtureCss } from "@stillwind-internal/tools-fixture-css/stillwind-css";
import { compileSource, NativeDesignSystemLoader, NativeProjectSession } from "@stillwind/native";
import type { NativePreparedSource, NativeStylesheetSelection } from "@stillwind/native";
import { expect, test, type BenchRunOptions } from "vite-plus/test";

const repeated: BenchRunOptions = {
  iterations: 9,
  time: 0,
  warmupIterations: 1,
  warmupTime: 0,
};
const minimal =
  "@utility flex {display:flex;} @utility block {display:block;} @utility red {color:red;} @stillwind-slot utilities;";
const full = resolveFixtureCss('@import "stillwind";');

for (const [name, css] of [
  ["minimal", minimal],
  ["full-registry", full],
] as const) {
  test(`loader/${name}`, async ({ bench, task }) => {
    task.meta.measurement = repeated;
    let output!: NativeDesignSystemLoader;
    await bench(`loader/${name}`, { async: false }, () => {
      output = loader(css);
    }).run(repeated);
    expect(output).toBeInstanceOf(NativeDesignSystemLoader);
  });

  test(`standalone/${name}`, async ({ bench, task }) => {
    const source = sources(1)[0];
    const input = {
      source: source.document.code,
      moduleId: source.moduleId,
      uri: source.document.uri,
      sourceMapSource: source.document.sourceMapSource!,
      language: "tsx" as const,
      moduleKind: "unambiguous" as const,
      css: {
        content: css,
        name: "root.css",
        uri: "untitled:root.css",
        sourceMapSource: "untitled:root.css",
      },
    };
    task.meta.measurement = repeated;
    let output!: ReturnType<typeof compileSource>;
    await bench(`standalone/${name}`, { async: false }, () => {
      output = compileSource(input, {});
    }).run(repeated);
    expect(output.diagnostics).toEqual([]);
  });
}

for (const calls of [1, 100, 1000]) {
  test(`unchanged/${calls}-calls`, async ({ bench, task }) => {
    const input = sources(1, calls);
    const session = new NativeProjectSession(loader(minimal), true);
    session.compile(input);
    task.meta.measurement = repeated;
    let output!: ReturnType<NativeProjectSession["compile"]>;
    await bench(`unchanged/${calls}-calls`, { async: false }, () => {
      output = session.compile(input);
    }).run(repeated);
    expect(output.modules).toHaveLength(1);
    expect(output.modules[0].diagnostics).toEqual([]);
  });
}

for (const count of [10, 100, 300, 1000]) {
  const input = sources(count);
  const options = {
    ...repeated,
    iterations: count === 1000 ? 3 : 9,
  };

  test(`graph/fresh/${count}`, async ({ bench, task }) => {
    const design = loader(minimal);
    const freshOptions = {
      ...options,
      iterations: count === 1000 ? 3 : 5,
    };
    task.meta.measurement = freshOptions;
    let output!: ReturnType<NativeProjectSession["compile"]>;
    await bench(`graph/fresh/${count}`, { async: false }, () => {
      output = new NativeProjectSession(design, true).compile(input);
    }).run(freshOptions);
    expect(output.modules).toHaveLength(count);
    expect(output.modules.every((module) => module.diagnostics.length === 0)).toBe(true);
    task.meta.fingerprint = createHash("sha256")
      .update(JSON.stringify(output.modules))
      .digest("hex");
  });

  test(`graph/unchanged/${count}`, async ({ bench, task }) => {
    const session = new NativeProjectSession(loader(minimal), true);
    session.compile(input);
    task.meta.measurement = options;
    let output!: ReturnType<NativeProjectSession["compile"]>;
    await bench(`graph/unchanged/${count}`, { async: false }, () => {
      output = session.compile(input);
    }).run(options);
    expect(output.modules).toHaveLength(count);
  });

  test(`graph/single-edit/${count}`, async ({ bench, task }) => {
    const session = new NativeProjectSession(loader(minimal), true);
    session.compile(input);
    let edit = 0;
    let output!: ReturnType<NativeProjectSession["compile"]>;
    task.meta.measurement = repeated;
    await bench(`graph/single-edit/${count}`, { async: false }, () => {
      output = session.compile([
        {
          ...input[0],
          document: {
            ...input[0].document,
            code: input[0].document.code.replaceAll("flex", ++edit % 2 ? "block" : "flex"),
          },
        },
      ]);
    }).run(repeated);
    expect(output.modules).toHaveLength(1);
    expect(output.modules[0].diagnostics).toEqual([]);
  });

  test(`css/plan/${count}`, async ({ bench, task }) => {
    const session = new NativeProjectSession(loader(minimal), true);
    session.compile(input);
    const selection: NativeStylesheetSelection = {
      modules: input.map((source) => source.moduleId),
      coverage: { kind: "observed" },
      placement: "document",
    };
    task.meta.measurement = repeated;
    let output!: ReturnType<NativeProjectSession["planStylesheet"]>;
    await bench(`css/plan/${count}`, { async: false }, () => {
      output = session.planStylesheet(selection);
    }).run(repeated);
    expect(output.producers).toHaveLength(count);
  });

  test(`css/carrier/${count}`, async ({ bench, task }) => {
    const session = new NativeProjectSession(loader(minimal), true);
    session.compile(input);
    task.meta.measurement = repeated;
    let output!: string;
    await bench(`css/carrier/${count}`, { async: false }, () => {
      output = session.moduleCss(input[0].moduleId).css;
    }).run(repeated);
    expect(output).not.toBe("");
  });
}

function loader(css: string) {
  const result = new NativeDesignSystemLoader();
  result.provide(0, "root.css", css, "untitled:root.css", "untitled:root.css");
  return result;
}

function sources(count: number, calls = 8): NativePreparedSource[] {
  return Array.from({ length: count }, (_, i) => ({
    kind: "direct",
    moduleId: `module-${i}.tsx`,
    document: {
      uri: `untitled:module-${i}.tsx`,
      sourceMapSource: `untitled:module-${i}.tsx`,
      format: "jsx",
      code:
        "import {style} from 'stillwind/react';\n" +
        Array.from({ length: calls }, (_, j) => `export const p${j}=style('flex red');`).join("\n"),
    },
    syntax: {
      language: "tsx",
      moduleKind: "unambiguous",
    },
  }));
}
