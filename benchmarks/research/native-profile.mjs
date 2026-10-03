import { createRequire } from "node:module";
const { NativeDesignSystemLoader, NativeProjectSession } = createRequire(import.meta.url)(
  "../../target/benchmark-research/profile.node",
);
const loader = new NativeDesignSystemLoader();
loader.provide(
  0,
  "root.css",
  "@utility flex {display:flex;} @utility block {display:block;} @utility red {color:red;} @stillwind-slot utilities;",
  "untitled:root.css",
  "untitled:root.css",
);
function input(count) {
  return Array.from({ length: count }, (_, i) => ({
    kind: "direct",
    moduleId: `module-${i}.tsx`,
    document: {
      uri: `untitled:module-${i}.tsx`,
      sourceMapSource: `untitled:module-${i}.tsx`,
      format: "jsx",
      code:
        "import {style} from 'stillwind/react';\n" +
        Array.from({ length: 8 }, (_, j) => `export const p${j}=style('flex red');`).join("\n"),
    },
    syntax: { language: "tsx", moduleKind: "unambiguous" },
  }));
}
function run(name, operation) {
  console.error("RESEARCH_CASE " + name);
  const start = performance.now();
  const result = operation();
  console.log(
    JSON.stringify({
      name,
      wallMs: performance.now() - start,
      modules: result.modules?.length,
      producers: result.producers?.length,
      cssBytes: result.css?.length,
    }),
  );
  if (result.modules?.some((module) => module.diagnostics.length)) {
    throw new Error("Compilation diagnostics");
  }
  return result;
}
for (const count of [100, 300, 1000]) {
  const sources = input(count);
  let session;
  for (let i = 0; i < 2; i++) {
    run(`fresh/${count}/${i}`, () => {
      session = new NativeProjectSession(loader, true);
      return session.compile(sources);
    });
  }
  for (let i = 0; i < 2; i++) {
    run(`unchanged/${count}/${i}`, () => session.compile(sources));
  }
  for (let i = 0; i < 3; i++) {
    run(`edit/${count}/${i}`, () =>
      session.compile([
        {
          ...sources[0],
          document: {
            ...sources[0].document,
            code: sources[0].document.code.replaceAll("flex", i % 2 ? "flex" : "block"),
          },
        },
      ]),
    );
  }
  for (let i = 0; i < 3; i++) {
    run(`plan/${count}/${i}`, () =>
      session.planStylesheet({
        modules: sources.map((source) => source.moduleId),
        coverage: { kind: "observed" },
        placement: "document",
      }),
    );
  }
}
