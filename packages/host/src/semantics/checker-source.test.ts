import { expect, test } from "vite-plus/test";

import type { TypeFactSource } from "./backend.ts";
import { CheckerSource, identityKey } from "./checker-source.ts";

test("checker declaration identity has its own namespace", () => {
  const key = identityKey({
    kind: "ref",
    moduleId: "src/a.tsx",
    start: 2,
    end: 6,
    bindingName: "item",
  });
  expect(Buffer.from(key.slice("stillwind-identity/v1:".length), "base64url").toString()).toBe(
    '{"kind":"ref","moduleId":"src/a.tsx","start":2,"end":6,"bindingName":"item"}',
  );
  expect(
    identityKey({ bindingName: "item", end: 6, start: 2, moduleId: "src/a.tsx", kind: "ref" }),
  ).toBe(key);
  expect(key).toMatch(/^stillwind-identity\/v1:/);
});

test("one authored producer can stamp distinct generated copies once each", () => {
  const original = "const value=style('flex');";
  const generated = `${original}\n${original}`;
  const callStart = original.indexOf("style("),
    callEnd = original.indexOf(")") + 1;
  const asset = {
    identity: {
      kind: "style" as const,
      moduleId: "Copied.vue",
      start: callStart,
      end: callEnd,
      bindingName: "value",
    },
    metadata: {
      label: null,
    },
    callStart,
    callEnd,
  };
  const source: TypeFactSource = {
    kind: "projected",
    moduleId: "Copied.vue",
    resolverId: "Copied.vue",
    checkerFileName: "Copied.vue",
    projectionFileName: "Copied.vue",
    document: {
      code: original,
      uri: "untitled:Copied.vue",
      format: "vue",
    },
    projection: {
      code: generated,
      syntax: {
        language: "ts",
        moduleKind: "module",
      },
      map: {
        kind: "volar",
        mappings: [],
      },
      converter: {
        name: "test",
        version: "1",
        optionsFingerprint: "1",
      },
      importRealms: [],
      editableRegions: [],
    },
    probes: [],
    checkerSpans: [0, original.length + 1].map((shift) => ({
      role: "producer",
      start: callStart,
      end: callEnd,
      projectedStart: callStart + shift,
      projectedEnd: callEnd + shift,
    })),
  };
  const shadow = new CheckerSource(
    source,
    {
      source: generated,
      language: "ts",
    },
    0,
    [asset, asset],
    new Map(),
  );
  expect(shadow.identities.size).toBe(1);
  expect(shadow.text.split(identityKey(asset.identity))).toHaveLength(3);
  expect(shadow.producers.size).toBe(2);
});

test("checker helper imports avoid authored identifiers and preserve authored calls", () => {
  const original = `const __stillwindCheckerHelper\\u0030=0,__stillwindCheckerHelper1=1;
function __VLS_unwrap(value:unknown){return String(value);}
__VLS_unwrap('authored');`;
  const generated = `${original}\n__VLS_unwrap(value, {} as import('vue').Ref<unknown>);`;
  const callee = generated.lastIndexOf("__VLS_unwrap");
  const source: TypeFactSource = {
    kind: "projected",
    moduleId: "HelperNames.vue",
    resolverId: "HelperNames.vue",
    checkerFileName: "HelperNames.vue",
    projectionFileName: "HelperNames.vue",
    document: { code: original, uri: "untitled:HelperNames.vue", format: "vue" },
    projection: {
      code: generated,
      syntax: { language: "ts", moduleKind: "module" },
      map: { kind: "volar", mappings: [] },
      converter: { name: "test", version: "1", optionsFingerprint: "1" },
      importRealms: [],
      editableRegions: [],
    },
    probes: [],
    checkerBindings: [
      {
        name: "__VLS_unwrap",
        projectedStart: callee,
        projectedEnd: callee + "__VLS_unwrap".length,
      },
    ],
  };
  const shadow = new CheckerSource(source, { source: generated, language: "ts" }, 0, [], new Map());
  expect(shadow.text).toContain(original);
  expect(shadow.text).toContain(
    "__stillwindCheckerHelper2(value, {} as import('vue').Ref<unknown>)",
  );
  expect(shadow.text).toContain("__VLS_unwrap as __stillwindCheckerHelper2");
});
