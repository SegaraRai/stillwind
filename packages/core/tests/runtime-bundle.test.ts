import { mkdtemp, rm, writeFile } from "node:fs/promises";
import path from "node:path";

import { build } from "vite-plus";
import { expect, test } from "vite-plus/test";

async function bundle(source: string, minify = false): Promise<string> {
  const root = await mkdtemp(path.join(import.meta.dirname, ".test-runtime-bundle-"));
  try {
    const entry = path.join(root, "entry.js");
    await writeFile(entry, source);
    const result = await build({
      configFile: false,
      logLevel: "silent",
      build: {
        write: false,
        minify,
        lib: {
          entry,
          formats: ["es"],
        },
      },
    });
    const output = Array.isArray(result) ? result[0] : result;
    if (!("output" in output)) {
      expect.fail("Expected a finite bundle");
    }
    return output.output
      .filter((file) => file.type === "chunk")
      .map((file) => file.code)
      .join("\n");
  } finally {
    await rm(root, { recursive: true, force: true });
  }
}

const formats = [
  ["ClassNameObject", "className"],
  ["ClassObject", "class"],
  ["ClassString", "class"],
] as const;

test("each packaged output format keeps only the requested style conversion", async () => {
  for (const [format] of formats) {
    const code = await bundle(
      `export { stillwind${format}StyleValue as convert } from "stillwind/runtime";`,
    );
    const runtime = await import(
      `data:text/javascript;base64,${Buffer.from(code).toString("base64")}`
    );
    expect(runtime.convert({ color: "red" })).toEqual(
      format === "ClassString" ? "color:red" : { color: "red" },
    );
    expect(code).not.toMatch(/mergeFragments|toFragment|ClassKey|CustomPropertiesKey|Symbol/);
  }
});

test("packaged production merges exclude validation, other formats, and unused conversion", async () => {
  const code = await bundle(
    'export { stillwindClassNameObjectMerge as merge, stillwindClassNameObjectClassKey as key } from "stillwind/runtime";',
  );
  const runtime = await import(
    `data:text/javascript;base64,${Buffer.from(code).toString("base64")}`
  );
  expect(runtime.merge({ [runtime.key]: "ready" })).toEqual({ className: "ready" });
  expect(code).not.toMatch(
    /function toFragment\b|Checked|NODE_ENV|different runtime instance|class-object|class-string/,
  );
});

test("development readers can be bundled without materialization", async () => {
  const code = await bundle(
    'export { stillwindRefClassChecked as read } from "stillwind/runtime/development";',
  );
  const runtime = await import(
    `data:text/javascript;base64,${Buffer.from(code).toString("base64")}`
  );
  expect(runtime.read({ c: "marker" })).toBe("marker");
  expect(() => runtime.read({})).toThrow(/class token/);
  expect(code).not.toMatch(/mergeFragments|toFragment|Symbol/);
});

test("packaged CVA factories select validation through the runtime entrypoint", async () => {
  for (const [format, classProperty] of formats) {
    for (const entry of ["stillwind/runtime", "stillwind/runtime/development"]) {
      const development = entry.endsWith("/development");
      const code = await bundle(`export { stillwind${format}Cva as cva } from "${entry}";`, true);
      const runtime = await import(
        `data:text/javascript;base64,${Buffer.from(code).toString("base64")}`
      );
      const choose = runtime.cva("base", {
        variants: {
          kind: {
            active: "selected",
          },
        },
        defaultVariants: {
          kind: "active",
        },
        compoundVariants: [
          {
            kind: "active",
            className: "compound",
          },
        ],
      });
      expect(choose({ class: "extra" })).toEqual({
        [classProperty]: "base selected compound extra",
      });
      if (development) {
        expect(() => choose({ class: Symbol("class") })).toThrow(/unsupported class value/);
      } else {
        expect(code).not.toMatch(/unsupported class value|different runtime instance/);
      }
    }
  }
}, 30_000);

test("minified output runtimes materialize fragments without configuration property names", async () => {
  for (const [format, classProperty] of formats) {
    for (const entry of ["stillwind/runtime", "stillwind/runtime/development"]) {
      const development = entry.endsWith("/development");
      const code = await bundle(
        `export { stillwind${format}Merge as merge, stillwind${format}ToFragment as convert } from "${entry}";`,
        true,
      );
      const runtime = await import(
        `data:text/javascript;base64,${Buffer.from(code).toString("base64")}`
      );
      const props = {
        [classProperty]: "card",
        style: {
          color: "red",
        },
        title: "ready",
      };
      expect(runtime.merge(runtime.convert(props))).toEqual({
        [classProperty]: "card",
      });
      if (development) {
        expect(() => runtime.convert(null)).toThrow(/materialized props object/);
        expect(() => runtime.merge(props)).toThrow(/materialized props object/);
      }
      expect(code).not.toMatch(/classProperty|styleFormat|ClassKey|StyleKey|CustomPropertiesKey/);
    }
  }
}, 30_000);
