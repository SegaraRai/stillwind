import { readdir, readFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { summarizeSource } from "@stillwind/native";
import { expect, test } from "vite-plus/test";

import type { TypeFactSource } from "./backend.ts";
import { BrowserTypeFactsService } from "./browser.ts";
import { resolveTypeFactsEnvironment, TypeScriptFactsService } from "./type-facts.ts";

test("browser TS6 and Node TS7 preserve the same finite declaration candidates", async () => {
  const root = fileURLToPath(new URL("../../", import.meta.url));
  const sourceCode = `import {style} from 'stillwind/react';
export const first = style.ref('first');
export const second = style.ref('second');
declare const condition: boolean;
const selected = condition ? first : second;
selected;
export const selectedStyle = style('block');
selectedStyle;`;
  const code = `${sourceCode}\nconst ordinary={value:42};\nordinary;\nordinary.value;\nconst factory=()=>42;\nfactory();\nconst attributes=style({'data-ready':true});\nconst composed=style(attributes,'block');\nattributes;`;
  const file = path.join(root, ".test-browser-facts.ts");
  const virtual = "/workspace/.test-browser-facts.ts";
  const assets = summarizeSource(code, "entry.ts", "ts", "module").assets;
  /**
   * Creates equivalent original-coordinate input for the Node or browser checker.
   * @param id - Checker and resolver path in the chosen environment.
   * @returns A direct TypeScript source using the shared original URI and probes.
   */
  const source = (
    id: string,
  ): Extract<
    TypeFactSource,
    {
      /** Selects source whose checker text is the original document. */
      kind: "direct";
    }
  > => ({
    kind: "direct",
    moduleId: "entry.ts",
    resolverId: id,
    checkerFileName: id,
    projectionFileName: id,
    document: {
      uri: "file:///workspace/entry.ts",
      code,
      format: "script",
    },
    syntax: {
      language: "ts",
      moduleKind: "module",
    },
    probes: [
      {
        start: sourceCode.lastIndexOf("selected;"),
        end: sourceCode.lastIndexOf("selected;") + 8,
      },
      {
        start: sourceCode.lastIndexOf("selectedStyle;"),
        end: sourceCode.lastIndexOf("selectedStyle;") + "selectedStyle".length,
      },
      ...["ordinary", "ordinary.value", "factory()", "attributes"].map((expression) => ({
        start: code.lastIndexOf(`${expression};`),
        end: code.lastIndexOf(`${expression};`) + expression.length,
      })),
    ],
  });
  const files = new Map<string, string>();
  const core = fileURLToPath(new URL("../../../core/", import.meta.url));
  await capture(core, "/workspace/node_modules/stillwind", files);
  const library = path.join(root, "node_modules/typescript-6/lib");
  for (const name of await readdir(library)) {
    if (name.endsWith(".d.ts")) {
      files.set(
        `/node_modules/typescript/lib/${name}`,
        await readFile(path.join(library, name), "utf8"),
      );
    }
  }
  const browser = new BrowserTypeFactsService({
    root: "/workspace",
    files,
    tsconfig: false,
  });
  const node = new TypeScriptFactsService(resolveTypeFactsEnvironment(root, false));
  try {
    const declarations = new Map([["entry.ts", assets]]);
    const actual = browser.collect([source(virtual)], declarations);
    const candidates = actual.get("entry.ts")?.[0].value?.value.candidates;
    expect(candidates?.kind).toBe("finite");
    if (candidates?.kind !== "finite") {
      expect.fail("expected a finite candidate set");
    }
    expect(candidates.values).toHaveLength(2);
    const styles = actual.get("entry.ts")?.[1].value;
    expect(styles?.kind).toBe("styles");
    if (styles?.value.candidates.kind !== "finite") {
      expect.fail("expected finite style candidates");
    }
    expect(styles.value.candidates.values).toHaveLength(1);
    expect(
      actual
        .get("entry.ts")!
        .slice(2, 5)
        .map(({ value }) => value),
    ).toStrictEqual([null, null, null]);
    expect(actual.get("entry.ts")![5].value?.kind).toBe("styles");
    expect(actual).toEqual(
      node.collect([source(file)], declarations, {
        overlayFiles: new Map([[file, code]]),
      }),
    );
    const widened = sourceCode.replace(
      "const selected =",
      "const selected: import('stillwind/react').StyleRef =",
    );
    const changed = {
      ...source(virtual),
      document: { ...source(virtual).document, code: widened },
      probes: [
        {
          start: widened.lastIndexOf("selected;"),
          end: widened.lastIndexOf("selected;") + 8,
        },
      ],
    };
    expect(
      browser.collectAvailable(
        [changed],
        new Map([["entry.ts", summarizeSource(widened, "entry.ts", "ts", "module").assets]]),
      ).unverified,
    ).not.toHaveLength(0);
  } finally {
    browser.close();
    node.close();
  }
});

/**
 * Copies package metadata and declarations into a browser inventory.
 * @param directory - Filesystem directory to traverse, skipping node_modules, src, and typecheck.
 * @param virtual - Absolute virtual directory under which captured paths are stored.
 * @param files - Mutable inventory receiving UTF-8 text from JSON and .d.ts files.
 * @returns Completion of recursive capture.
 * @throws If enumeration or reading fails.
 */
async function capture(
  directory: string,
  virtual: string,
  files: Map<string, string>,
): Promise<void> {
  for (const entry of await readdir(directory, { withFileTypes: true })) {
    if (entry.name === "node_modules" || entry.name === "src" || entry.name === "typecheck") {
      continue;
    }
    const absolute = path.join(directory, entry.name);
    if (entry.isDirectory()) {
      await capture(absolute, `${virtual}/${entry.name}`, files);
    } else if (entry.name.endsWith(".json") || entry.name.endsWith(".d.ts")) {
      files.set(`${virtual}/${entry.name}`, await readFile(absolute, "utf8"));
    }
  }
}
