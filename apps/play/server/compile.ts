import { readFile, realpath } from "node:fs/promises";
import { resolve } from "node:path";
import { pathToFileURL } from "node:url";

import { loadCssGraph, type CompilerOutput } from "@stillwind/host";
import { NativeProjectSession } from "@stillwind/native";
import type { ViteDevServer } from "vite-plus";

export async function compilePlayground(
  source: string,
  theme: string,
  css: string,
  server: ViteDevServer,
): Promise<CompilerOutput> {
  const resolveImport = async (specifier: string, importer: string) => {
    const result = await server.pluginContainer.resolveId(specifier, importer);
    if (!result || result.external) {
      throw new Error(`Cannot resolve CSS ${specifier} from ${importer}`);
    }
    return result.id;
  };
  const { loader, resources } = await loadCssGraph(
    await realpath(resolve(server.config.root, css)),
    {},
    {
      resolve: async (specifier, importer) => realpath(await resolveImport(specifier, importer)),
      read: (path) => readFile(path, "utf8"),
      documentUri: (id) => pathToFileURL(id).href,
      sourceMapSource: (id) => pathToFileURL(id).href,
      watch: () => {},
    },
    theme,
  );
  const session = new NativeProjectSession(loader, false);
  const project = session.compile([
    {
      kind: "direct",
      moduleId: "Play.tsx",
      document: {
        uri: "untitled:Play.tsx",
        format: "jsx",
        // Preserve byte length for the Play shorthand's source positions.
        code: source.replace(/\bexport App;/g, "export{App}"),
      },
      syntax: {
        language: "tsx",
        moduleKind: "unambiguous",
      },
    },
  ]);
  const output = project.modules[0]!;
  output.css = session.css();
  for (const resource of resources) {
    const file = await resolveImport(resource.specifier, resource.sourceName);
    const url = `/@fs/${file
      .replaceAll("\\", "/")
      .split("/")
      .map((segment) =>
        encodeURIComponent(segment)
          .replaceAll("'", "%27")
          .replaceAll("(", "%28")
          .replaceAll(")", "%29"),
      )
      .join("/")}`;
    output.css = output.css.replaceAll(resource.placeholder, JSON.stringify(url).slice(1, -1));
  }
  return output;
}
