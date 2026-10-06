import path from "node:path";

import { transform as compileAstro } from "@astrojs/compiler";
import { compileProject, scanSource } from "@stillwind/native";
import { checkerEnvironment, prepareSource, type PreparedSource } from "@stillwind/projection";
import { afterAll, beforeAll, describe, expect, test } from "vite-plus/test";

import { TypeScriptFactsService } from "../../src/semantics/index.ts";

const root = path.resolve(import.meta.dirname, "../../../integration/testss");
const filename = path.join(root, "Projection.astro");
const macroImport = "import {style} from 'stillwind/astro';";
const baseDeclaration = "const base=style('flex');";
const stylesheet = {
  content: "@utility flex {display:flex;} @utility grid {display:grid;} @stillwind-slot utilities;",
  name: "theme.css",
  uri: "untitled:theme.css",
};

function component(script: string, template = "") {
  return `---\n// 日本語 🧭\n${macroImport}${baseDeclaration}${script}\n---\n${template}`;
}

function originalRange(code: string, text: string, from = 0) {
  const start = code.indexOf(text, from);
  expect(start).toBeGreaterThanOrEqual(0);
  return {
    start: Buffer.byteLength(code.slice(0, start)),
    end: Buffer.byteLength(code.slice(0, start + text.length)),
  };
}

describe("Astro @astrojs/compiler 4.0.0", () => {
  let facts: TypeScriptFactsService;

  beforeAll(() => {
    facts = new TypeScriptFactsService({
      root,
      project: {
        kind: "isolated",
      },
    });
  });

  afterAll(() => facts?.close());

  async function project(code: string) {
    const prepared = await prepareSource({
      moduleId: "Projection.astro",
      resolverId: filename,
      checkerFileName: filename,
      projectionFileName: "Projection.astro",
      document: { uri: "untitled:Projection.astro", format: "astro", code },
      syntax: { language: "ts", moduleKind: "module" },
    });
    if (prepared.kind !== "projected") {
      expect.fail("an Astro document must produce a projection");
    }
    return prepared;
  }

  function collect(prepared: PreparedSource) {
    const scan = scanSource(prepared);
    if (scan.kind !== "ready") {
      expect.fail(JSON.stringify(scan.batch.reports));
    }
    const summary = scan.summary;
    const typeFacts = facts
      .collect(
        [
          {
            ...prepared,
            ...checkerEnvironment(prepared),
            probes: summary.typeProbes,
            checkerSpans: summary.checkerSpans,
            checkerBindings: summary.checkerBindings,
          },
        ],
        new Map([[prepared.moduleId, summary.assets]]),
      )
      .get(prepared.moduleId)!;
    return { summary, typeFacts };
  }

  async function compile(prepared: PreparedSource) {
    const collected = collect(prepared);
    const module = compileProject([{ ...prepared, typeFacts: collected.typeFacts }], stylesheet)
      .modules[0];
    expect(module.diagnostics).toStrictEqual([]);
    if (module.code === null) {
      expect.fail("a compiled Astro document must contain code");
    }
    const compiled = await compileAstro(module.code, { filename: prepared.projectionFileName });
    expect(compiled.diagnostics.filter(({ severity }) => severity === 1)).toStrictEqual([]);
    return module;
  }

  test("frontmatter generic functions retain original UTF-8 identities", async () => {
    const code = component(
      "function identity<T>(value:T):T{return value;}const derived=style(identity(base),'grid');",
    ).replaceAll("\n", "\r\n");
    const prepared = await project(code);
    expect(prepared.projection.converter).toMatchObject({
      name: "@astrojs/compiler",
      version: "4.0.0",
    });
    expect(prepared.projection.syntax).toStrictEqual({ language: "tsx", moduleKind: "module" });
    expect(
      prepared.projection.importRealms.map(({ start, end }) => ({ start, end })),
    ).toStrictEqual([{ start: 3, end: code.indexOf("\r\n---", 3) + 2 }]);
    const module = await compile(prepared);
    expect(module.composition).toMatchObject([
      { base: { moduleId: prepared.moduleId, ...originalRange(code, "style('flex')") } },
    ]);
    expect(module.code).toContain("function identity<T>(value:T):T{return value;}");
    expect(module.code).toContain("// 日本語 🧭\r\n");
    expect(module.css).toMatch(/display:\s*grid/);
  });

  test("template fragments retain generic callback types and runtime attributes", async () => {
    const code = component(
      "function identity<T>(value:T):T{return value;}const label='日本語 🧭';",
      "{[base].map(item=><><div {...style(identity(item),'grid',{class:label})}/><span>{label}</span></>)}",
    );
    const module = await compile(await project(code));
    expect(module.composition).toMatchObject([{ base: originalRange(code, "style('flex')") }]);
    expect(module.code).toContain("[base].map(item=><>");
    expect(module.code).toContain("{label}</span></>");
    expect(module.css).toMatch(/display:\s*grid/);
  });

  test("frontmatter returns preserve types used by the following template", async () => {
    const code = component(
      "interface Props{enabled:boolean}if(Astro.props.enabled)return Astro.redirect('/');",
      "<div {...style(base,'grid')}/>",
    );
    const module = await compile(await project(code));
    expect(module.composition).toMatchObject([{ base: originalRange(code, "style('flex')") }]);
    expect(module.code).toContain("if(Astro.props.enabled)return Astro.redirect('/');");
  });

  for (const attributes of [
    "",
    ' type="module"',
    " is:inline",
    ' data-label="authored"',
    ' define:vars={{label:"authored"}}',
  ]) {
    test(`script${attributes}: multiline clients retain their UTF-16 regions and UTF-8 identities`, async () => {
      const body = `\r\n// 日本語 🧭\r\n${macroImport}\r\n${baseDeclaration}\r\nconst derived=style(base,'grid');\r\nconsole.log(derived);\r\n`;
      const prefix = `<p>日本語 🧭</p>\r\n<script${attributes}>`;
      const code = `${prefix}${body}</script>`;
      const region = { start: prefix.length, end: prefix.length + body.length };
      const prepared = await project(code);
      expect(prepared.projection.checkerScriptRanges).toStrictEqual([region]);
      expect(
        prepared.projection.importRealms.map(({ start, end }) => ({ start, end })),
      ).toStrictEqual([region]);
      expect(prepared.projection.editableRegions).toStrictEqual([
        { ...region, importRealm: prepared.projection.importRealms[0].id },
      ]);
      const buffers = checkerEnvironment(prepared).checkerBuffers!;
      expect(buffers).toMatchObject([
        { language: "tsx" },
        { source: body, language: "ts", region },
      ]);
      expect(buffers[0].source.length).toBe(prepared.projection.code.length);
      expect(
        [...buffers[0].source.matchAll(/\r\n|\r|\n/g)].map((match) => [match.index, match[0]]),
      ).toStrictEqual(
        [...prepared.projection.code.matchAll(/\r\n|\r|\n/g)].map((match) => [
          match.index,
          match[0],
        ]),
      );
      const module = await compile(prepared);
      expect(module.composition).toMatchObject([{ base: originalRange(code, "style('flex')") }]);
      expect(module.code).toContain(prefix);
      expect(module.code).toContain("console.log(derived);");
      expect(module.css).toMatch(/display:\s*grid/);
    });
  }

  test("client generic arrow functions retain their finite identities", async () => {
    const code = `<script>\n${macroImport}${baseDeclaration}const identity=<T,>(value:T)=>value;const derived=style(identity(base),'grid');\n</script>`;
    const module = await compile(await project(code));
    expect(module.composition).toMatchObject([{ base: originalRange(code, "style('flex')") }]);
    expect(module.code).toContain("const identity=<T,>(value:T)=>value;");
  });

  test("rejects client syntax that the converter copies into invalid TSX", async () => {
    const arrow = "const identity=<T>(value:T)=>value;";
    const prepared = await project(
      `<script>${macroImport}${baseDeclaration}${arrow}const derived=style(identity(base),'grid');</script>`,
    );
    expect(prepared.projection.code).toContain(arrow);
    const result = scanSource(prepared);
    expect(result).toMatchObject({
      kind: "failed",
      batch: {
        module: prepared.moduleId,
        reports: [
          {
            code: "SW10007",
            primary: {
              location: {
                uri: prepared.document.uri,
              },
            },
            notes: expect.arrayContaining(["Converter: @astrojs/compiler@4.0.0"]),
          },
        ],
      },
    });
  });

  test("server and identical client bodies keep distinct local declaration identities", async () => {
    const body = `\n// 日本語 🧭\n${macroImport}${baseDeclaration}const derived=style(base,'grid');console.log(derived);\n`;
    const code =
      component("const derived=style(base,'grid');", "<div {...derived}/>") +
      `<script>${body}</script><script is:inline>${body}</script>`;
    const prepared = await project(code);
    const buffers = checkerEnvironment(prepared).checkerBuffers!;
    const clientStarts = [code.indexOf(body), code.lastIndexOf(body)];
    expect(buffers.slice(1)).toStrictEqual(
      clientStarts.map((start) => ({
        source: body,
        language: "ts",
        region: { start, end: start + body.length },
      })),
    );
    expect(new Set(prepared.projection.importRealms.map(({ id }) => id)).size).toBe(3);
    expect(prepared.projection.importRealms.map(({ parent }) => parent)).toStrictEqual([
      undefined,
      undefined,
      undefined,
    ]);
    const module = await compile(prepared);
    const expected = [...code.matchAll(/style\('flex'\)/g)].map(({ index }) =>
      originalRange(code, "style('flex')", index),
    );
    expect(
      module.composition
        .map(({ base: { start, end } }) => ({ start, end }))
        .sort((a, b) => a.start - b.start),
    ).toStrictEqual(expected);
  });

  test("an authored client function keeps its binding beside the server macro import", async () => {
    const client = "function style(value){return value;}const authored=style('authored');";
    const code = component("", `<div {...style(base,'grid')}/><script>${client}</script>`);
    const module = await compile(await project(code));
    expect(module.composition).toMatchObject([{ base: originalRange(code, "style('flex')") }]);
    expect(module.code).toContain(client);
  });

  for (const { name, annotation, issueKind, errorText, errorOffset } of [
    {
      name: "incompatible reassignment",
      annotation: "",
      issueKind: "assignment",
      errorText: "selected=other",
      errorOffset: 0,
    },
    {
      name: "erased identity",
      annotation: ":unknown",
      issueKind: "identity",
      errorText: "style(selected,'grid')",
      errorOffset: 6,
    },
  ]) {
    test(`client ${name} reports its original UTF-8 use`, async () => {
      const code = `<p>日本語 🧭</p>\r\n<script>\r\n${macroImport}${baseDeclaration}const other=style('grid');let selected${annotation}=base;if(Math.random())selected=other;const derived=style(selected,'grid');\r\n</script>`;
      const prepared = await project(code);
      const range = originalRange(code, "selected", code.indexOf(errorText) + errorOffset);
      expect(() => collect(prepared)).toThrow(
        expect.objectContaining({
          issue: expect.objectContaining({ kind: issueKind }),
          moduleId: prepared.moduleId,
          ...range,
        }),
      );
    });
  }

  test("JSON, external and empty scripts preserve their text without client checker buffers", async () => {
    const scripts = [
      '<script type="application/json">{"call":"style(\'grid\')","label":"日本語 🧭"}</script>',
      '<script type="application/ld+json">{"name":"日本語 🧭"}</script>',
      '<script src="./client.ts"></script>',
      "<script is:inline></script>",
    ];
    const code = component("", `<div {...style(base,'grid')}/>${scripts.join("\n")}`);
    const prepared = await project(code);
    expect(prepared.projection.checkerScriptRanges).toStrictEqual([]);
    expect(checkerEnvironment(prepared).checkerBuffers).toStrictEqual([
      { source: prepared.projection.code, language: "tsx" },
    ]);
    const module = await compile(prepared);
    expect(module.composition).toMatchObject([{ base: originalRange(code, "style('flex')") }]);
    for (const script of scripts) {
      expect(module.code).toContain(script);
    }
  });

  test("an empty frontmatter leaves client imports in their own realm", async () => {
    const code = `---\r\n---\r\n<script>${macroImport}${baseDeclaration}const derived=style(base,'grid');</script>`;
    const prepared = await project(code);
    expect(prepared.projection.importRealms).toHaveLength(2);
    const module = await compile(prepared);
    expect(module.composition).toMatchObject([{ base: originalRange(code, "style('flex')") }]);
    expect(module.code).toContain("---\r\n---\r\n<script>");
  });
});
