import { createRequire } from "node:module";

const { NativeDesignSystemLoader, NativeProjectSession } = createRequire(import.meta.url)(
  "../../target/benchmark-research/profile.node",
);

for (const count of [10, 100, 1000, 10_000]) {
  const loader = new NativeDesignSystemLoader();
  loader.provide(
    0,
    "root.css",
    "@utility flex {display:flex;} @utility block {display:block;} @stillwind-slot utilities;" +
      Array.from(
        { length: count },
        (_, i) => `@utility probe-${i} {color:rgb(${i % 256} 0 0);}`,
      ).join("\n"),
    "untitled:root.css",
    "untitled:root.css",
  );
  const input = Array.from({ length: 100 }, (_, i) => ({
    kind: "direct",
    moduleId: `module-${i}.ts`,
    document: {
      uri: `untitled:module-${i}.ts`,
      sourceMapSource: `untitled:module-${i}.ts`,
      format: "jsx",
      code: "import {style} from 'stillwind/react'; export const props=style('flex');",
    },
    syntax: { language: "ts", moduleKind: "unambiguous" },
  }));
  const session = new NativeProjectSession(loader, true);
  console.error(`RESEARCH_CASE registry/fresh/${count}`);
  session.compile(input);
  for (let edit = 0; edit < 3; edit++) {
    console.error(`RESEARCH_CASE registry/edit/${count}/${edit}`);
    const start = performance.now();
    const output = session.compile([
      {
        ...input[0],
        document: {
          ...input[0].document,
          code: input[0].document.code.replace("'flex'", edit % 2 ? "'flex'" : "'block'"),
        },
      },
    ]);
    console.log(JSON.stringify({ count, edit, wallMs: performance.now() - start }));
    if (output.modules.some((module) => module.diagnostics.length)) {
      throw new Error("Compilation diagnostics");
    }
  }
}
