// Executes independently rendered folded and reference expressions in fresh environments.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

const cases = JSON.parse(readFileSync(0, "utf8"));
const runtimes = new Map();
for (const framework of new Set(cases.map((item) => item.framework))) {
  runtimes.set(framework, await import(`../../../packages/core/src/runtime/${framework}.ts`));
}
let count = 0;
for (const mode of ["development", "production"]) {
  process.env.NODE_ENV = mode;
  for (const item of cases) {
    const runtime = runtimes.get(item.framework);
    for (const truth of [false, true]) {
      for (const fail of [0, 2, 3]) {
        for (const valueKind of [
          "number",
          "boolean",
          "text",
          "url",
          "coercion",
          "getter",
          "throwing-coercion",
        ]) {
          const execute = (code) => {
            const log = [];
            const state = { current: 0 };
            const value = (index) => {
              log.push(index);
              state.current += index;
              if (index === fail) throw new Error(`failure:${index}`);
              switch (valueKind) {
                case "number":
                  return index;
                case "boolean":
                  return index % 2 === 0;
                case "text":
                  return '` ${text} " \\ \n ; :';
                case "url":
                  return new URL(`https://example.test/${index}`);
                case "getter":
                  return {
                    get active() {
                      log.push(`getter:${index}`);
                      return truth;
                    },
                  };
                case "coercion":
                  return {
                    toString() {
                      log.push(`convert:${index}`);
                      return String(index);
                    },
                  };
              }
            };
            const test = () => {
              log.push("test");
              return truth;
            };
            const base = () => {
              log.push("base");
              if (truth)
                return new Proxy(
                  { style: { "--old": "old" }, "data-base": 1 },
                  {
                    ownKeys(target) {
                      log.push("keys");
                      return Reflect.ownKeys(target);
                    },
                    get(target, key) {
                      log.push(`get:${String(key)}`);
                      return target[key];
                    },
                  },
                );
              return null;
            };
            const payload = () => {
              log.push("payload");
              return truth ? { "--x": "old" } : null;
            };
            try {
              const result = new Function(
                ...Object.keys(runtime),
                "value",
                "test",
                "base",
                "payload",
                "state",
                `return (${code});`,
              )(...Object.values(runtime), value, test, base, payload, state);
              return {
                result: JSON.parse(
                  JSON.stringify(result, (_key, entry) =>
                    typeof entry === "function" ? "[function]" : entry,
                  ),
                ),
                keys: Object.keys(result),
                styleKeys:
                  result.style && typeof result.style === "object"
                    ? Object.keys(result.style)
                    : null,
                log,
              };
            } catch (error) {
              return { error: [error.constructor.name, error.message], log };
            }
          };
          assert.deepEqual(
            execute(item.optimized),
            execute(item.reference),
            JSON.stringify({ ...item, mode, truth, fail, valueKind }),
          );
          count++;
        }
      }
    }
  }
}
console.log(`${count} generated-JS differential executions passed`);
