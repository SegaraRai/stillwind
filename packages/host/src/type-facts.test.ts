import path from "node:path";
import { fileURLToPath } from "node:url";

import { summarizeSource } from "@stillwind/native";
import { expect, test } from "vite-plus/test";

import { identityKey, type TypeFactSource } from "./checker-source.ts";
import { collectTypeScriptFacts, TypeFactsError, type AssetRecord } from "./type-facts.ts";

const root = path.dirname(fileURLToPath(import.meta.url));

function collect(expressions: readonly string[], recognitionOnly = false) {
  const declarations = `import { style } from '../../core/src/frameworks/react.ts';
import type { StyleRef } from '../../core/src/macro/index.ts';
const a = style.ref('a');
const b = style.ref('b');
declare const enabled: boolean;
const alias = a;
type Alias = typeof a;
const typedAlias: Alias = alias;
function preserve<T>(value: T): T { return value; }
const generic = preserve(alias);
const selected: typeof a | typeof b = enabled ? generic : b;
const variant = a('hover:');
const scoped = a('hover:').in(b);
const container = a.at('width > 40rem');
const projection = a.asAnchor();
declare const widened: StyleRef;
`;
  let code = declarations;
  const probes = expressions.map((expression) => {
    const start = Buffer.byteLength(code);
    code += `${expression};\n`;
    return { start, end: start + Buffer.byteLength(expression), recognitionOnly };
  });
  const moduleId = "ref-facts.ts";
  const assets: AssetRecord[] = ["a", "b"].map((bindingName) => {
    const start = declarations.indexOf(`const ${bindingName} =`) + "const ".length;
    const callStart = declarations.indexOf(`style.ref('${bindingName}')`);
    return {
      identity: { kind: "ref", moduleId, start, end: start + 1, bindingName },
      metadata: { label: bindingName },
      callStart,
      callEnd: callStart + `style.ref('${bindingName}')`.length,
    };
  });
  const fileName = path.join(root, moduleId);
  const source: TypeFactSource = {
    kind: "direct",
    moduleId,
    resolverId: fileName,
    checkerFileName: fileName,
    projectionFileName: fileName,
    document: { uri: `untitled:${moduleId}`, code, format: "script" },
    syntax: { language: "ts", moduleKind: "module" },
    probes,
  };
  return {
    facts: collectTypeScriptFacts([source], new Map([[moduleId, assets]]), root).get(moduleId)!,
    assets,
    probes,
  };
}

test("ref facts preserve aliases, generic returns, and finite unions", () => {
  const { facts, assets, probes } = collect(["alias", "typedAlias", "generic", "selected"]);
  expect(facts).toStrictEqual(
    probes.map((probe, index) => ({
      ...probe,
      value: {
        kind: "refs",
        value: {
          kind: "ref",
          candidates: {
            kind: "finite",
            values: index === 3 ? assets.map((asset) => asset.identity) : [assets[0].identity],
          },
        },
      },
    })),
  );
});

test("recognition facts identify declarations beside ref uses and widened types", () => {
  const { facts, assets, probes } = collect(
    ["alias", "variant", "scoped", "container", "projection", "widened"],
    true,
  );
  expect(facts).toStrictEqual([
    {
      ...probes[0],
      value: {
        kind: "refs",
        value: { kind: "ref", candidates: { kind: "finite", values: [assets[0].identity] } },
      },
    },
  ]);
});

test("required ref facts reject a widened declaration identity", () => {
  expect(() => collect(["widened"])).toThrow(TypeFactsError);
});

test("imported ref and scope aliases preserve independent finite candidates through a barrel", () => {
  const modules = new Map([
    [
      "scope-facts-producer.ts",
      `import {style} from 'stillwind/react';
export const first = style.ref('first');
export const second = style.ref('second');
export const outer = style.ref('outer');
export const inner = style.ref('inner');`,
    ],
    [
      "scope-facts-chooser.ts",
      `import {first, second, outer, inner} from './scope-facts-producer';
declare const enabled: boolean;
function preserve<T>(value: T): T { return value; }
type RefAlias = typeof first | typeof second;
type ScopeAlias = typeof outer | typeof inner;
export const selectedRef: RefAlias = preserve(enabled ? first : second);
export const selectedScope: ScopeAlias = preserve(enabled ? outer : inner);`,
    ],
    [
      "scope-facts-barrel.ts",
      `export {selectedRef as reference, selectedScope as scope} from './scope-facts-chooser';`,
    ],
    [
      "scope-facts-consumer.ts",
      `import {style} from 'stillwind/react';
import {reference as importedRef, scope as importedScope} from './scope-facts-barrel';
function preserve<T>(value: T): T { return value; }
const refAlias = preserve(importedRef);
const scopeAlias = preserve(importedScope);
export const result = style({[refAlias('hover').in(scopeAlias)]: 'flex'});`,
    ],
  ]);
  const consumer = modules.get("scope-facts-consumer.ts")!;
  const operands = ["refAlias", "scopeAlias"];
  const probes = operands.map((operand) => {
    const start = consumer.lastIndexOf(operand);
    return { start, end: start + operand.length, recognitionOnly: false };
  });
  const sources: TypeFactSource[] = [...modules].map(([moduleId, code]) => {
    const fileName = path.join(root, moduleId);
    return {
      kind: "direct",
      moduleId,
      resolverId: fileName,
      checkerFileName: fileName,
      projectionFileName: fileName,
      document: { uri: `untitled:${moduleId}`, code, format: "script" },
      syntax: { language: "ts", moduleKind: "module" },
      probes: moduleId === "scope-facts-consumer.ts" ? probes : [],
    };
  });
  const assets = new Map(
    sources.map((source) => [
      source.moduleId,
      summarizeSource(source.document.code, source.moduleId, "ts", "module").assets,
    ]),
  );
  const identities = assets.get("scope-facts-producer.ts")!.map((asset) => asset.identity);
  const facts = collectTypeScriptFacts(sources, assets, root).get("scope-facts-consumer.ts")!;
  expect(facts).toStrictEqual(
    probes.map((probe, index) => ({
      ...probe,
      value: {
        kind: "refs",
        value: {
          kind: "ref",
          candidates: { kind: "finite", values: identities.slice(index * 2, index * 2 + 2) },
        },
      },
    })),
  );
});

function collectBrands(expressions: readonly string[]) {
  const moduleId = "brand-facts.ts";
  let code = `import {style} from 'stillwind/react';
const base=style('flex'),other=style('grid'),target=style.ref('target');
const frames=style.keyframes({from:'opacity-0'}),transition=style.viewTransition({old:'opacity-0'});
type Anonymous<T> = {[K in keyof T]: T[K]};
declare const anonymous:Anonymous<typeof base>;
declare const selected:Anonymous<typeof base>|Anonymous<typeof other>;
declare const widened:{readonly 'data-__stillwindStyleIdentity':string};
declare const unregistered:{readonly 'data-__stillwindStyleIdentity':'missing'};
declare const mixed:typeof base|typeof target;
declare const mappedTransition:Anonymous<typeof transition>;
const refUse=target.asAnchor(),variant=target('hover:'),animationUse=frames(''),transitionUse=transition.for('x');
`;
  const assets = summarizeSource(code, moduleId, "ts", "module").assets;
  code += `declare const crossKind:{readonly 'data-__stillwindStyleIdentity':${JSON.stringify(identityKey(assets.find((asset) => asset.identity.kind === "ref")!.identity))}};\n`;
  const probes = expressions.map((expression) => {
    const start = Buffer.byteLength(code);
    code += `${expression};\n`;
    return { start, end: start + Buffer.byteLength(expression) };
  });
  const id = path.join(root, moduleId);
  const source: TypeFactSource = {
    kind: "direct",
    moduleId,
    resolverId: id,
    checkerFileName: id,
    projectionFileName: id,
    document: {
      code,
      uri: "untitled:brands",
      format: "script",
    },
    syntax: {
      language: "ts",
      moduleKind: "module",
    },
    probes,
  };
  return {
    facts: collectTypeScriptFacts([source], new Map([[moduleId, assets]]), root).get(moduleId)!,
    assets,
  };
}

test("anonymous branded styles and transitions retain finite identities", () => {
  const { facts, assets } = collectBrands(["anonymous", "selected", "mappedTransition"]);
  expect(facts.map((fact) => fact.value.kind)).toStrictEqual([
    "styles",
    "styles",
    "viewTransitions",
  ]);
  const styles = assets.filter((asset) => asset.identity.kind === "style");
  expect(facts.map((fact) => fact.value.value.candidates)).toStrictEqual([
    { kind: "finite", values: [styles[0].identity] },
    { kind: "finite", values: styles.map((asset) => asset.identity) },
    {
      kind: "finite",
      values: [assets.find((asset) => asset.identity.kind === "viewTransition")!.identity],
    },
  ]);
});

for (const expression of ["widened", "unregistered", "mixed", "crossKind"]) {
  test(`branded ${expression} reports an unresolved declaration identity`, () => {
    expect(() => collectBrands([expression])).toThrow(/finite set of declaration identities/);
  });
}

test("unique-symbol brands distinguish declarations from ref, animation, and transition uses", () => {
  const { facts } = collectBrands([
    "target",
    "frames",
    "mappedTransition",
    "refUse",
    "variant",
    "animationUse",
    "transitionUse",
  ]);
  expect(facts.map((fact) => fact.value.kind)).toStrictEqual([
    "refs",
    "keyframes",
    "viewTransitions",
  ]);
});
