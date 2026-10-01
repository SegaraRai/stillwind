import path from "node:path";
import { fileURLToPath } from "node:url";

import { composeModuleSourceMap, moduleSourceMap } from "@stillwind/host";
import type { LoaderContext } from "webpack";

import { createProject, type TurbopackLoaderOptions } from "./project.ts";

export default function (
  this: LoaderContext<TurbopackLoaderOptions>,
  source: string,
  inputMap: any,
) {
  const callback = this.async();
  if (!/stillwind\/(?:react|preact|solid|vue|svelte|astro|module-css)/.test(source)) {
    return callback(null, source, inputMap);
  }
  void (async () => {
    const project = await createProject(this);
    try {
      const request = `./${path.basename(this.resourcePath)}?stillwind-module-css`;
      const output = await project.compiler.compile(source, this.resourcePath, project.host, {
        moduleCssImport: request,
      });
      if (!output?.code) {
        return callback(null, source, inputMap);
      }
      if (new URLSearchParams(this.resourceQuery).has("stillwind-module-css")) {
        const plan = project.compiler.native.planStylesheet({
          modules: [output.moduleId],
          coverage: { kind: "observed" },
          placement: "document",
        });
        const imports: string[] = [];
        const replacements: string[] = [];
        for (const resource of [...project.loaded.resources, ...plan.resources]) {
          const file = await project.resolveAsset(
            resource.specifier,
            "moduleId" in resource
              ? project.compiler.resolverId(resource.moduleId)
              : resource.sourceName,
          );
          project.host.watch(file);
          let request = path.relative(path.dirname(this.resourcePath), file).replaceAll("\\", "/");
          if (!request.startsWith(".")) {
            request = `./${request}`;
          }
          const binding = `asset${imports.length}`;
          imports.push(`import ${binding} from ${JSON.stringify(request)};`);
          replacements.push(
            `.replaceAll(${JSON.stringify(resource.placeholder)},JSON.stringify(typeof ${binding}==='string'?${binding}:${binding}.src).slice(1,-1))`,
          );
        }
        return callback(
          null,
          `${imports.join("\n")}\nexport const parts=${JSON.stringify(plan.parts.map(({ key, css }) => ({ key, css })))}.map(part=>({...part,css:part.css${replacements.join("")}}));export const css=parts.map(part=>part.css).join("");export default css;`,
        );
      }
      let code = output.code;
      if (!output.hasModuleCssImport && project.options.compiler.automatic !== false) {
        let carrier = path
          .relative(
            path.dirname(this.resourcePath),
            fileURLToPath(
              new URL(
                "./stillwind-turbopack.css",
                import.meta.resolve("@stillwind/integration/turbopack"),
              ),
            ),
          )
          .replaceAll("\\", "/");
        if (!carrier.startsWith(".")) {
          carrier = `./${carrier}`;
        }
        code += `\nimport ${JSON.stringify(carrier)};\n`;
      }
      callback(
        null,
        code,
        composeModuleSourceMap(
          moduleSourceMap(source, output.code, this.resourcePath, output.mappings, [], ""),
          inputMap,
        ) as Parameters<typeof callback>[2],
      );
    } finally {
      project.compiler.close();
    }
  })().catch(callback);
}
