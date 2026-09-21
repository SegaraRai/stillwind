import assert from "node:assert/strict";
import fs from "node:fs";
import { createRequire } from "node:module";
import path from "node:path";
import { performance } from "node:perf_hooks";
import { pathToFileURL, fileURLToPath } from "node:url";

import { projectFrameworkSource } from "../../packages/projection/src/index.ts";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");
const local = createRequire(import.meta.url);
const hostRequire = createRequire(path.join(root, "packages/host/package.json"));
const viteRequire = createRequire(path.join(root, "packages/integration/package.json"));
const ts = local("typescript");
const { API } = await import(pathToFileURL(hostRequire.resolve("typescript/unstable/sync")));
const guards7 = await import(pathToFileURL(hostRequire.resolve("typescript/unstable/ast/is")));
const { TraceMap, decodedMappings } = local("@jridgewell/trace-mapping");
const norm = (p) => path.resolve(p).replaceAll("\\", "/");
const workspace = norm(path.join(root, "packages/integration/test/.sfc-checker-virtual"));
const report = {
  versions: {
    ts6: ts.version,
    ts7: hostRequire("typescript/package.json").version,
    vue: viteRequire("vue/package.json").version,
    svelte: local("svelte/package.json").version,
    astro: viteRequire("astro/package.json").version,
    vueLanguageCore: local("@vue/language-core/package.json").version,
    svelte2tsx: local("svelte2tsx/package.json").version,
    astroCompiler: local("@astrojs/compiler/package.json").version,
  },
  rows: [],
};
const visit = (node, fn) => {
  fn(node);
  node.forEachChild((child) => visit(child, fn));
};
const lines = (text) => [0, ...[...text.matchAll(/\n/g)].map((m) => m.index + 1)];

function projectionPoints(input) {
  if (!input.fw_code) {
    return null;
  }
  const points = new Map();
  if (input.fw2js_map.kind === "volar") {
    for (const m of input.fw2js_map.mappings) {
      for (let i = 0; i < m.sourceOffsets.length; i++) {
        const length = m.lengths[i];
        const generatedLength = m.generatedLengths?.[i] ?? length;
        points.set(m.generatedOffsets[i], m.sourceOffsets[i]);
        if (length === generatedLength) {
          for (let j = 1; j < length; j++) {
            points.set(m.generatedOffsets[i] + j, m.sourceOffsets[i] + j);
          }
        }
      }
    }
  } else {
    const map = new TraceMap(input.fw2js_map.json);
    const generatedLines = lines(input.source),
      originalLines = lines(input.fw_code);
    for (const [line, segments] of decodedMappings(map).entries()) {
      for (const s of segments) {
        if (s.length >= 4) {
          points.set(generatedLines[line] + s[0], originalLines[s[2]] + s[3]);
        }
      }
    }
  }
  return points;
}

function wrap(framework, script, template, moduleScript = "") {
  if (framework === "astro") {
    return `---\r\n${moduleScript}\r\n${script}\r\n---\r\n${template}`;
  }
  const module = moduleScript
    ? `<script ${framework === "svelte" ? 'context="module" ' : ""}lang="ts">${moduleScript}</script>\r\n`
    : "";
  return (
    module +
    `<script ${framework === "vue" ? "setup " : ""}lang="ts">\r\n${script}\r\n</script>\r\n` +
    (framework === "vue" ? `<template>${template}</template>` : template)
  );
}

function fixtures(framework, variant) {
  const imports = `import { style } from 'stillwind/${framework}';\nimport { remote, remote2, pass } from './barrel';\n`;
  let body = `// 日本語 🧭\nconst a = style('text-red-500');\nconst b = style('text-blue-500');\nconst flag = Math.random() > 0.5;\nfunction observe<T>(name: string, value: T): T { return value; }\nfunction choose<T>(value: T): T { return value; }\n`;
  const probes = [];
  const add = (name, expression, expected) => {
    body += `observe('${name}', ${expression});\n`;
    probes.push({ name, expression, expected });
  };
  add("local", "flag ? a : b", ["a", "b"]);
  add("transitive", "pass(flag ? remote : remote2)", ["remote", "remote2"]);
  body += `let selected: typeof a | typeof b = a;\nif (flag) selected = b;\nfunction readSelected() { return selected; }\n`;
  add("mutable", "readSelected()", ["a", "b"]);
  add("generic", "choose(flag ? a : b)", ["a", "b"]);
  body += `const object = { selected: flag ? a : b };\n`;
  add("property", "object.selected", ["a", "b"]);
  add("module-realm", "moduleBase", ["moduleBase"]);
  body += `const r1 = style.ref(); const r2 = style.ref();\nconst k1 = style.keyframes({from:'opacity-0',to:'opacity-100'});\nconst k2 = style.keyframes({from:'opacity-100',to:'opacity-0'});\nconst v1 = style.viewTransition({}); const v2 = style.viewTransition({});\n`;
  add("refs", "flag ? r1 : r2", ["r1", "r2"]);
  add("keyframes", "flag ? k1 : k2", ["k1", "k2"]);
  add("transitions", "flag ? v1 : v2", ["v1", "v2"]);
  if (variant === "invalid") {
    body += `let invalid = a;\nif (flag) invalid = b;\n`;
    add("invalid", "invalid", ["a"]);
  }
  if (variant === "erased") {
    body += `const erased: any = a;\n`;
    add("erased", "erased", null);
    body += `const widened: import('stillwind/${framework}').StyleResult<string> = a;\n`;
    add("widened", "widened", null);
    body += `function forget(value: import('stillwind/${framework}').StyleResult<string>) { return value; }\n`;
    add("erased-parameter", "forget(a)", null);
  }
  if (framework === "vue") {
    body += `const { enabled } = defineProps<{enabled: boolean}>();\nconst choices = [a, b];\n`;
  }
  if (framework === "svelte") {
    body += `let { enabled }: {enabled: boolean} = $props();\nlet reactive = $state(a);\nimport { writable } from 'svelte/store';\nconst store = writable(flag ? a : b);\nconst choices = [a, b];\n`;
  }
  if (framework === "astro") {
    body += `interface Props {enabled: boolean}\nconst {enabled} = Astro.props;\nconst choices = [a, b];\n`;
  }
  const expression = framework === "svelte" ? "$store" : "enabled ? a : b";
  probes.push({ name: "template", expression, expected: ["a", "b"] });
  const call = `style(observe('template', ${expression}), 'font-bold')`;
  let template = framework === "vue" ? `<div v-bind="${call}"/>` : `<div {...${call}}></div>`;
  const loopCall = `style(observe('loop', item), 'italic')`;
  probes.push({ name: "loop", expression: "item", expected: ["a", "b"] });
  template +=
    framework === "vue"
      ? `<div v-for="item in choices" v-bind="${loopCall}"/>`
      : framework === "svelte"
        ? `{#each choices as item}<div {...${loopCall}}></div>{/each}`
        : `{choices.map(item => <div {...${loopCall}} />)}`;
  if (framework === "svelte") {
    probes.push({ name: "rune", expression: "reactive", expected: ["a"] });
    template += `<div {...style(observe('rune', reactive), 'underline')}></div>`;
  }
  const producer = `import {style} from 'stillwind/${framework}';\nexport const remote = style('font-bold');\nexport const remote2 = style('italic');`;
  return {
    files: new Map([
      [
        `Entry.${framework}`,
        wrap(
          framework,
          imports + body,
          template,
          `import {style as moduleStyle} from 'stillwind/${framework}'; export const moduleBase = moduleStyle('underline');`,
        ),
      ],
      [
        "barrel.ts",
        `export {remote,remote2} from './Producer.${framework}'; export {pass} from './helper';`,
      ],
      ["helper.ts", "export function pass<T>(x:T):T {return x;}"],
      [`Producer.${framework}`, wrap(framework, "", "<div/>", producer)],
    ]),
    probes,
  };
}

async function prepare(framework, variant, stripTemplateSymbols = false) {
  const directory = `${workspace}/${framework}-${variant}`;
  const { files: originals, probes } = fixtures(framework, variant);
  const files = new Map(),
    modules = [],
    identities = new Set();
  const queue = [`Entry.${framework}`],
    visited = new Set();
  while (queue.length) {
    const name = queue.shift();
    if (visited.has(name)) {
      continue;
    }
    visited.add(name);
    const original = originals.get(name);
    assert.ok(original !== undefined, name);
    const projected = await projectFrameworkSource({
      id: `${directory}/${name}`,
      path: name,
      source: original,
    });
    const points = projectionPoints(projected);
    const virtual = `${directory}/${name}${projected.fw_code ? "." + projected.language : ""}`;
    const ast = ts.createSourceFile(
      virtual,
      projected.source,
      ts.ScriptTarget.Latest,
      true,
      projected.language === "tsx" ? ts.ScriptKind.TSX : ts.ScriptKind.TS,
    );
    const edits = [];
    visit(ast, (node) => {
      if (
        stripTemplateSymbols &&
        ts.isCallExpression(node) &&
        node.expression.getText(ast) === "style"
      ) {
        const sourcePosition = points?.get(node.getStart(ast));
        if (sourcePosition !== undefined && sourcePosition > original.lastIndexOf("</script>")) {
          edits.push({ at: node.getStart(ast), end: node.getStart(ast), text: "__researchProps(" });
          edits.push({ at: node.end, end: node.end, text: ")" });
        }
      }
      if (
        (ts.isImportDeclaration(node) || ts.isExportDeclaration(node)) &&
        node.moduleSpecifier &&
        ts.isStringLiteral(node.moduleSpecifier)
      ) {
        const specifier = node.moduleSpecifier.text;
        if (specifier.startsWith(".")) {
          const dependency =
            specifier.slice(2) + (/\.(vue|svelte|astro)$/.test(specifier) ? "" : ".ts");
          queue.push(dependency);
          const suffix = /\.(vue|svelte|astro)$/.test(dependency)
            ? framework === "vue"
              ? ".ts"
              : ".tsx"
            : "";
          edits.push({
            at: node.moduleSpecifier.getStart(ast),
            end: node.moduleSpecifier.end,
            text: JSON.stringify("./" + dependency + suffix),
          });
        }
      }
      if (
        !ts.isVariableDeclaration(node) ||
        !ts.isIdentifier(node.name) ||
        !node.initializer ||
        !ts.isCallExpression(node.initializer)
      ) {
        return;
      }
      const call = node.initializer;
      if (
        !/^(?:style|moduleStyle)(?:\.(?:ref|keyframes|viewTransition))?$/.test(
          call.expression.getText(ast),
        )
      ) {
        return;
      }
      const originalStart = points ? points.get(node.name.getStart(ast)) : node.name.getStart(ast);
      assert.equal(
        original.slice(originalStart, originalStart + node.name.text.length),
        node.name.text,
      );
      const identity = `${name}:${Buffer.byteLength(original.slice(0, originalStart))}:${node.name.text}`;
      identities.add(identity);
      edits.push({
        at: call.end - 1,
        end: call.end - 1,
        text: `${call.arguments.length ? "," : ""}undefined as unknown as {readonly __stillwindDeclarationIdentity:${JSON.stringify(identity)}}`,
      });
    });
    edits.sort((a, b) => a.at - b.at);
    let shadow = projected.source;
    for (const e of [...edits].reverse()) {
      shadow = shadow.slice(0, e.at) + e.text + shadow.slice(e.end);
    }
    if (stripTemplateSymbols) {
      shadow +=
        '\ndeclare function __researchProps<T>(value:T): Omit<T, keyof import("stillwind/macro").StyleResult<never, {}>>;\n';
    }
    const toProjected = (offset) => {
      let delta = 0;
      for (const e of edits) {
        const start = e.at + delta;
        if (offset < start) {
          break;
        }
        if (offset < start + e.text.length) {
          return null;
        }
        delta += e.text.length - (e.end - e.at);
      }
      return offset - delta;
    };
    const mapPoint = (offset) => {
      const p = toProjected(offset);
      return p === null ? null : points ? (points.get(p) ?? null) : p;
    };
    files.set(norm(virtual), shadow);
    modules.push({ name, virtual, original, shadow, mapPoint });
  }
  const shims =
    framework === "svelte"
      ? [
          local.resolve("svelte2tsx/svelte-shims-v4.d.ts"),
          local.resolve("svelte2tsx/svelte-jsx-v4.d.ts"),
        ]
      : framework === "astro"
        ? [path.join(path.dirname(viteRequire.resolve("astro/package.json")), "env.d.ts")]
        : [];
  const config = `${directory}/tsconfig.json`;
  const options = {
    strict: true,
    noEmit: true,
    skipLibCheck: true,
    target: "esnext",
    module: "esnext",
    moduleResolution: "bundler",
    jsx: "preserve",
    allowImportingTsExtensions: true,
    types: [],
  };
  const roots = [modules[0].virtual, ...shims];
  files.set(norm(config), JSON.stringify({ compilerOptions: options, files: roots }));
  return { directory, config, options, roots, files, modules, probes, identities };
}

function inspect(prepared, program, checker, guards, version) {
  const result = { probes: [], diagnostics: [] };
  const entry = prepared.modules[0];
  const sf = program.getSourceFile(entry.virtual);
  assert.ok(sf, entry.virtual);
  const is7 = version === 7;
  const typeArgs = (type) =>
    is7
      ? type.getAliasTypeArguments().length
        ? type.getAliasTypeArguments()
        : type.isTypeReference()
          ? checker.getTypeArguments(type)
          : []
      : (type.aliasTypeArguments ??
        (type.objectFlags & ts.ObjectFlags.Reference ? checker.getTypeArguments(type) : []));
  function candidates(type) {
    const values = [];
    for (const t of (type.isUnionType ? type.isUnionType() : type.isUnion())
      ? is7
        ? type.getTypes()
        : type.types
      : [type]) {
      const symbol = is7 ? (t.getAliasSymbol() ?? t.getSymbol()) : (t.aliasSymbol ?? t.getSymbol());
      if (
        !["StyleResult", "StyleRef", "StyleKeyframes", "StyleViewTransition"].includes(symbol?.name)
      ) {
        return null;
      }
      const idType = typeArgs(t)[0];
      if (!idType) {
        return null;
      }
      const ids = (is7 ? idType.isUnionType() : idType.isUnion())
        ? is7
          ? idType.getTypes()
          : idType.types
        : [idType];
      for (const id of ids) {
        if (
          !(is7 ? id.isStringLiteralType() : id.isStringLiteral()) ||
          !prepared.identities.has(id.value)
        ) {
          return null;
        }
        values.push(id.value.split(":").at(-1));
      }
    }
    return [...new Set(values)].sort((left, right) => (left < right ? -1 : left > right ? 1 : 0));
  }
  visit(sf, (node) => {
    if (
      !guards.isCallExpression(node) ||
      node.arguments.length !== 2 ||
      !guards.isStringLiteral(node.arguments[0])
    ) {
      return;
    }
    const name = node.arguments[0].text;
    const expected = prepared.probes.find((p) => p.name === name);
    if (!expected) {
      return;
    }
    const expression = node.arguments[1],
      start = expression.getStart(sf),
      end = expression.end;
    const type = checker.getTypeAtLocation(expression);
    const mapped = [];
    for (let i = start; i < end; i++) {
      const point = entry.mapPoint(i);
      if (point !== null) {
        mapped.push(point);
      }
    }
    const originalStart =
      entry.original.indexOf(`observe('${name}', `) + `observe('${name}', `.length;
    const mappedRange = mapped.length ? [Math.min(...mapped), Math.max(...mapped) + 1] : null;
    const mappingValid =
      !!mappedRange &&
      mappedRange[0] === originalStart &&
      mappedRange[1] === originalStart + expected.expression.length;
    result.probes.push({
      name,
      type: checker.typeToString(type),
      candidates: candidates(type),
      expected: expected.expected?.toSorted() ?? null,
      mappingValid,
      originalRange: mappedRange,
      originalByteRange: mappedRange?.map((p) => Buffer.byteLength(entry.original.slice(0, p))),
    });
  });
  for (const module of prepared.modules) {
    const source = program.getSourceFile(module.virtual);
    assert.ok(source, `transitive module omitted: ${module.virtual}`);
    const diags = is7
      ? [
          ...program.getSyntacticDiagnostics(module.virtual),
          ...program.getSemanticDiagnostics(module.virtual),
        ]
      : [...program.getSyntacticDiagnostics(source), ...program.getSemanticDiagnostics(source)];
    for (const d of diags) {
      const start = is7 ? d.pos : d.start;
      result.diagnostics.push({
        module: module.name,
        code: d.code,
        text: is7 ? d.text : ts.flattenDiagnosticMessageText(d.messageText, " "),
        start,
        original: module.mapPoint(start),
      });
    }
  }
  return result;
}

function run6(p) {
  const started = performance.now();
  const options = ts.convertCompilerOptionsFromJson(p.options, p.directory).options;
  const host = ts.createCompilerHost(options);
  const originalRead = host.readFile,
    originalExists = host.fileExists,
    originalDirectory = host.directoryExists;
  host.readFile = (f) => p.files.get(norm(f)) ?? originalRead(f);
  host.fileExists = (f) => p.files.has(norm(f)) || originalExists(f);
  host.directoryExists = (f) =>
    [...p.files.keys()].some((key) => key.startsWith(norm(f) + "/")) || originalDirectory(f);
  const program = ts.createProgram(p.roots, options, host);
  return {
    ...inspect(p, program, program.getTypeChecker(), ts, 6),
    elapsedMs: performance.now() - started,
  };
}

function run7(p) {
  const started = performance.now();
  const loaded = new Set();
  const api = new API({
    cwd: p.directory,
    fs: {
      readFile: (f) => {
        const key = norm(f);
        if (p.files.has(key)) {
          loaded.add(key);
        }
        return p.files.get(key);
      },
      fileExists: (f) => (p.files.has(norm(f)) ? true : undefined),
      directoryExists: (f) =>
        [...p.files.keys()].some((key) => key.startsWith(norm(f) + "/")) ? true : undefined,
    },
  });
  let snapshot;
  try {
    snapshot = api.updateSnapshot({ openProjects: [p.config] });
    const project = snapshot.getProject(p.config);
    assert.ok(project, "TS7 project unavailable");
    const result = inspect(p, project.program, project.checker, guards7, 7);
    assert.ok(
      p.modules.every((m) => loaded.has(norm(m.virtual))),
      "all transitive virtual modules must be read",
    );
    return {
      ...result,
      loaded: p.modules.filter((m) => loaded.has(norm(m.virtual))).map((m) => m.name),
      elapsedMs: performance.now() - started,
    };
  } finally {
    snapshot?.dispose();
    api.close();
  }
}

function incremental7(p) {
  const api = new API({
    cwd: p.directory,
    fs: {
      readFile: (f) => p.files.get(norm(f)),
      fileExists: (f) => (p.files.has(norm(f)) ? true : undefined),
      directoryExists: (f) =>
        [...p.files.keys()].some((key) => key.startsWith(norm(f) + "/")) ? true : undefined,
    },
  });
  let snapshot;
  const update = (changes) => {
    const next = api.updateSnapshot(changes);
    snapshot?.dispose();
    snapshot = next;
    const project = snapshot.getProject(p.config);
    assert.ok(project);
    return project;
  };
  try {
    let project = update({ openProjects: [p.config] });
    const initial = inspect(p, project.program, project.checker, guards7, 7);
    const producer = p.modules.find((m) => m.name.startsWith("Producer."));
    const oldIdentity = [...p.identities].find((i) => i.endsWith(":remote2"));
    const newIdentity = oldIdentity.replace(/remote2$/, "remote3");
    p.files.set(
      norm(producer.virtual),
      p.files.get(norm(producer.virtual)).replace(oldIdentity, newIdentity),
    );
    p.identities.delete(oldIdentity);
    p.identities.add(newIdentity);
    project = update({ fileChanges: { changed: [producer.virtual] } });
    const changed = inspect(p, project.program, project.checker, guards7, 7);
    const helper = p.modules.find((m) => m.name === "helper.ts");
    const added = `${p.directory}/late.ts`;
    p.files.set(norm(added), "export function pass<T>(x:T):T{return x;}");
    p.files.set(norm(helper.virtual), "export {pass} from './late';");
    project = update({ fileChanges: { changed: [helper.virtual], created: [added] } });
    const afterAdd = inspect(p, project.program, project.checker, guards7, 7);
    for (const r of [initial, changed, afterAdd]) {
      assert.deepEqual(r.diagnostics, []);
    }
    p.files.delete(norm(added));
    project = update({ fileChanges: { deleted: [added] } });
    const missing = project.program
      .getSemanticDiagnostics(helper.virtual)
      .map((d) => ({ code: d.code, text: d.text }));
    const transitive = (r) => r.probes.find((probe) => probe.name === "transitive").candidates;
    assert.deepEqual(transitive(initial), ["remote", "remote2"]);
    assert.deepEqual(transitive(changed), ["remote", "remote3"]);
    assert.deepEqual(transitive(afterAdd), ["remote", "remote3"]);
    assert.ok(missing.some((d) => d.code === 2307));
    return {
      initial: transitive(initial),
      changed: transitive(changed),
      added: transitive(afterAdd),
      deleted: missing,
    };
  } finally {
    snapshot?.dispose();
    api.close();
  }
}

async function astroClient(split) {
  const p = await prepare("astro", "valid");
  const clientCode = `import {style} from 'stillwind/astro';\nconst client = style('underline');\nfunction observe<T>(name:string,value:T):T{return value;}\nobserve('client', client);`;
  const original = `---\r\n// 日本語 🧭\r\nconst server=1;\r\n---\r\n<div>{server}</div>\r\n<script>${clientCode}</script>`;
  const base = original.indexOf(clientCode);
  const converterSource = split
    ? original.slice(0, base) +
      clientCode.replace(/[^\r\n]/g, " ") +
      original.slice(base + clientCode.length)
    : original;
  const projection = await projectFrameworkSource({
    id: "Client.astro",
    path: "Client.astro",
    source: converterSource,
  });
  const points = projectionPoints({ ...projection, fw_code: original });
  const virtual = `${p.directory}/Client.astro.tsx`;
  const source = split ? clientCode : projection.source;
  const ast = ts.createSourceFile(
    "client.tsx",
    source,
    ts.ScriptTarget.Latest,
    true,
    ts.ScriptKind.TSX,
  );
  let call;
  visit(ast, (node) => {
    if (ts.isVariableDeclaration(node) && node.name.getText(ast) === "client") {
      call = node.initializer;
    }
  });
  assert.ok(call);
  const identity = `Client.astro:${Buffer.byteLength(original.slice(0, base + clientCode.indexOf("client =")))}:client`;
  const insert = `,undefined as unknown as {readonly __stillwindDeclarationIdentity:${JSON.stringify(identity)}}`;
  const at = call.end - 1;
  const shadow = source.slice(0, at) + insert + source.slice(at);
  const mapPoint = (offset) => {
    if (offset >= at && offset < at + insert.length) {
      return null;
    }
    const before = offset >= at ? offset - insert.length : offset;
    return split ? base + before : (points.get(before) ?? null);
  };
  p.modules = split
    ? [
        {
          name: "Client.astro#script",
          virtual: `${p.directory}/Client.script.ts`,
          original,
          shadow,
          mapPoint,
        },
        {
          name: "Client.astro",
          virtual,
          original,
          shadow: projection.source,
          mapPoint: (offset) => points.get(offset) ?? null,
        },
      ]
    : [{ name: "Client.astro", virtual, original, shadow, mapPoint }];
  p.files = new Map(p.modules.map((m) => [norm(m.virtual), m.shadow]));
  p.roots = [...p.modules.map((m) => m.virtual), ...p.roots.filter((f) => f.endsWith(".d.ts"))];
  p.files.set(norm(p.config), JSON.stringify({ compilerOptions: p.options, files: p.roots }));
  p.probes = [{ name: "client", expression: "client", expected: ["client"] }];
  p.identities = new Set([identity]);
  return p;
}

for (const framework of ["vue", "svelte", "astro"]) {
  for (const variant of ["valid", "invalid", "erased"]) {
    for (const stripTemplateSymbols of framework === "svelte" ? [false, true] : [false]) {
      const prepared = await prepare(framework, variant, stripTemplateSymbols);
      const row = {
        framework,
        variant,
        stripTemplateSymbols,
        modules: prepared.modules.map((m) => m.name),
      };
      for (const [name, run] of [
        ["ts6", run6],
        ["ts7", run7],
      ]) {
        try {
          row[name] = run(prepared);
        } catch (e) {
          row[name] = { error: e.stack };
        }
      }
      report.rows.push(row);
      console.log(
        framework,
        variant,
        stripTemplateSymbols ? "omit-style-identity" : "baseline",
        JSON.stringify(
          Object.fromEntries(
            ["ts6", "ts7"].map((v) => [
              v,
              row[v].error ?? {
                probes: row[v].probes.length,
                bad: row[v].probes.filter(
                  (p) =>
                    JSON.stringify(p.candidates) !== JSON.stringify(p.expected) || !p.mappingValid,
                ),
                diagnostics: row[v].diagnostics.map((d) => ({
                  code: d.code,
                  original: d.original,
                })),
              },
            ]),
          ),
        ),
      );
    }
  }
}
report.incremental = [];
for (const framework of ["vue", "svelte", "astro"]) {
  try {
    report.incremental.push({
      framework,
      ...incremental7(await prepare(framework, "valid", framework === "svelte")),
    });
  } catch (e) {
    report.incremental.push({ framework, error: e.stack });
  }
}
report.astroClient = [];
for (const split of [false, true]) {
  const p = await astroClient(split);
  report.astroClient.push({ split, ts6: run6(p), ts7: run7(p) });
}
fs.writeFileSync(
  new URL("./results/checker-comparison.json", import.meta.url),
  JSON.stringify(report, null, 2) + "\n",
);
for (const row of report.rows) {
  for (const version of ["ts6", "ts7"]) {
    const result = row[version];
    assert.equal(result.error, undefined);
    assert.ok(result.probes.length > 0);
    const expectedCount =
      11 +
      (row.framework === "svelte" ? 1 : 0) +
      (row.variant === "invalid" ? 1 : row.variant === "erased" ? 3 : 0);
    assert.equal(result.probes.length, expectedCount);
    for (const probe of result.probes) {
      assert.deepEqual(
        probe.candidates,
        probe.expected,
        `${version}/${row.framework}/${probe.name}`,
      );
      assert.equal(probe.mappingValid, true, `${version}/${row.framework}/${probe.name} mapping`);
    }
    const expectedCodes = row.variant === "invalid" ? [2322] : [];
    assert.deepEqual(
      result.diagnostics.map((d) => d.code).sort((a, b) => a - b),
      expectedCodes,
    );
    if (row.variant === "invalid") {
      assert.ok(Number.isInteger(result.diagnostics.find((d) => d.code === 2322).original));
    }
  }
}
for (const row of report.incremental) {
  assert.equal(row.error, undefined);
}
for (const row of report.astroClient) {
  for (const version of ["ts6", "ts7"]) {
    if (row.split) {
      assert.deepEqual(row[version].diagnostics, []);
      assert.deepEqual(row[version].probes[0].candidates, ["client"]);
      assert.equal(row[version].probes[0].mappingValid, true);
    } else {
      assert.ok(row[version].diagnostics.some((d) => d.code === 1232));
    }
  }
}
console.log(
  `Verified ${report.rows.length} scenarios on both checker versions and ${report.incremental.length} TS7 incremental sequences.`,
);
