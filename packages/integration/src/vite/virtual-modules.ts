import { readFile } from "node:fs/promises";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";

import { cleanModuleId, decodeCssSourceMap } from "@stillwind/host";
import type { Plugin } from "vite";

import { minifyStylesheet } from "./css-minify.ts";
import { readableFile } from "./file-access.ts";
import {
  fragmentPrefix,
  producerPrefix,
  projectionPrefix,
  runtimeId,
  runtimeSpecifier,
  valuePrefix,
  virtualCssId,
} from "./module-ids.ts";
import type { ViteProject } from "./project.ts";

/**
 * Public specifier for the packaged development runtime, resolved through the published Vite
 * entrypoint.
 */
const runtimeEntry = "virtual:stillwind/runtime-entry";

/** Public specifier for the optional runtime that renders development CSS source maps. */
const sourceMapEntry = "virtual:stillwind/runtime-source-map";

/**
 * Locates a packaged runtime module beside the published Vite entrypoint.
 *
 * @param file - Runtime basename without an extension, such as `runtime` or `runtime-source-map`.
 * @returns Absolute path to the runtime JavaScript file.
 */
function runtimeFile(file: string) {
  return fileURLToPath(new URL(`./${file}.js`, import.meta.resolve("@stillwind/integration/vite")));
}

/**
 * Creates hooks that resolve registered Stillwind references and load their generated modules.
 *
 * Unregistered references fail through the hook context. Projections compile source for CSS without
 * importing its application JavaScript.
 *
 * @param project - Configured project whose environment registry owns the references and producer
 * inputs.
 * @returns Resolution and loading hooks for runtime, producer, projection, fragment, CSS value, and
 * stylesheet snapshot modules.
 */
export function virtualModuleHooks(project: ViteProject): Pick<Plugin, "resolveId" | "load"> {
  const {
    settings,
    inputs,
    cssForBrowser,
    moduleRegistry,
    producerSpecifier,
    documentPlan,
    publish,
  } = project;

  return {
    /**
     * Resolves runtime aliases and issued references in the current environment's registry.
     * @param id - Authored request or resolved virtual identity.
     * @param importer - Full importer identity required for registered projection edges.
     * @returns A resolved virtual identity or packaged runtime path, or undefined for unrelated requests.
     * @throws If a projection or source-backed reference is unregistered.
     */
    async resolveId(id, importer) {
      const registry = moduleRegistry(this.environment?.name);
      if (id === runtimeEntry) {
        return runtimeFile("runtime");
      }
      if (id === sourceMapEntry) {
        return runtimeFile("runtime-source-map");
      }
      if (id.endsWith("?stillwind.css")) {
        const registered = registry.projection(id, importer);
        if (!registered) {
          this.error("Unregistered Stillwind projection");
        }
        return registered.replace("virtual:stillwind/", "\0stillwind:");
      }
      if (id === runtimeSpecifier || id === virtualCssId || id === runtimeId) {
        return runtimeId;
      }
      if (id === "virtual:stillwind/snapshot.css" || id === "\0stillwind:snapshot.css") {
        return "\0stillwind:snapshot.css";
      }
      if (/^(?:virtual:stillwind\/|\0stillwind:)(?:value|fragment|producer|projection):/.test(id)) {
        if (!registry.get(id)) {
          this.error("Unregistered Stillwind module");
        }
        return id.replace("virtual:stillwind/", "\0stillwind:");
      }
      return undefined;
    },
    /**
     * Loads generated runtime, producer, projection, fragment, and explicit CSS-value modules.
     * @param id - Resolved virtual identity issued or recognized by this environment.
     * @returns Generated JavaScript or snapshot CSS and its map, or undefined for unrelated modules.
     * @throws If a reference is invalid, source or resource access fails, or CSS planning or compilation fails.
     */
    async load(id) {
      const name = this.environment?.name;
      const registry = moduleRegistry(name);
      const entry = registry.get(id);
      if (
        [producerPrefix, projectionPrefix, fragmentPrefix, valuePrefix].some((prefix) =>
          id.startsWith(prefix),
        ) &&
        !entry
      ) {
        this.error("Unregistered Stillwind module");
      }
      if (id === "\0stillwind:snapshot.css") {
        const plan = await documentPlan(name, this);
        return decodeCssSourceMap(
          cssForBrowser(
            name,
            {
              css: plan.originalCss,
              sourceMap: plan.sourceMap,
            },
            plan.resourceReplacements,
          ),
        );
      }
      if (id.startsWith(producerPrefix)) {
        if (!settings.watching) {
          return "// Stillwind producer (vite)\nexport {};\n";
        }
        if (entry?.kind !== "producer") {
          this.error("Invalid Stillwind producer");
        }
        const producer = entry.moduleId;
        return `// Stillwind producer (vite)
import { register, synchronize, unregister } from ${JSON.stringify(runtimeSpecifier)};

await synchronize(${project.environment(name).revision});
register(${JSON.stringify(producer)});
if (import.meta.hot) {
  import.meta.hot.prune(() => {
    unregister(${JSON.stringify(producer)});
  });
}
`;
      }
      if (id.startsWith(projectionPrefix)) {
        if (entry?.kind !== "projection") {
          this.error("Invalid Stillwind projection");
        }
        const source = entry.sourceId;
        const file = await readableFile(source, settings.devServerRef);
        // Vite's addWatchFile also creates a JavaScript dependency during import analysis.
        const context = settings.watching
          ? {
              resolve: this.resolve.bind(this),
              warn: this.warn.bind(this),
              addWatchFile: (id: string) => {
                settings.devServerRef?.watcher.add(cleanModuleId(id));
              },
            }
          : this;
        context.addWatchFile(source);
        const result = await project
          .environment(name)
          .compileProjection(await readFile(file, "utf8"), source, context);
        await publish(name, context);
        return {
          code: `// Stillwind projection (vite)\nimport ${JSON.stringify(producerSpecifier(source, name))};\n`,
          meta: {
            stillwind: {
              ...result?.meta.stillwind,
              projection: true,
            },
          },
        };
      }
      if (id === runtimeId) {
        if (!settings.watching) {
          return "// Stillwind runtime (vite)\nexport {};\n";
        }
        const snapshot = await project.snapshot(name, this);
        const endpoint = `${settings.publicBase}__stillwind/css?environment=${encodeURIComponent(name ?? "client")}`;
        return `// Stillwind runtime (vite)
import { createDevelopmentRuntime } from ${JSON.stringify(runtimeEntry)};
${settings.devSourceMaps ? `import { renderCssWithSourceMap } from ${JSON.stringify(sourceMapEntry)};` : ""}

const runtime = createDevelopmentRuntime({
  snapshot: ${JSON.stringify(snapshot)},
  endpoint: ${JSON.stringify(endpoint)},
  ${settings.devSourceMaps ? "renderCss: renderCssWithSourceMap," : ""}
});

export const register = runtime.register;
export const unregister = runtime.unregister;
export const synchronize = runtime.synchronize;

if (import.meta.hot) {
  import.meta.hot.on("stillwind:css", runtime.apply);
  import.meta.hot.accept();
  import.meta.hot.dispose(() => {
    import.meta.hot.off("stillwind:css", runtime.apply);
  });
}
`;
      }
      if (id.startsWith(fragmentPrefix)) {
        if (entry?.kind !== "fragment") {
          this.error("Invalid Stillwind fragment");
        }
        let css = entry.css;
        if (!settings.watching && settings.minifyCss) {
          css = minifyStylesheet(css, settings, this).css;
        }
        const resources = [...entry.resources];
        for (const [, resource] of resources) {
          await readableFile(resource.id, settings.devServerRef);
        }
        const imports = resources.map(
          ([, resource], index) =>
            `import asset${index} from ${JSON.stringify(resource.id + (resource.id.includes("?") ? "&url" : "?url"))};\n`,
        );
        if (settings.watching && settings.devSourceMaps) {
          return [
            "// Stillwind CSS fragment (vite)\n",
            ...imports,
            `import { replaceCssResources } from ${JSON.stringify(sourceMapEntry)};\n`,
            `export const css = replaceCssResources(${JSON.stringify(css)}, [${resources.map(([placeholder], index) => `[${JSON.stringify(placeholder)}, JSON.stringify(asset${index}).slice(1, -1)]`).join(",")}]);\n`,
          ].join("");
        }
        const replacements = resources
          .map(
            ([placeholder], index) =>
              `.replaceAll(${JSON.stringify(placeholder)}, JSON.stringify(asset${index}).slice(1, -1))`,
          )
          .join("");
        return [
          "// Stillwind CSS fragment (vite)\n",
          ...imports,
          "\n",
          `export const css = ${JSON.stringify(css)}${replacements};\n`,
        ].join("");
      }
      if (id.startsWith(valuePrefix)) {
        if (entry?.kind !== "value") {
          this.error("Invalid Stillwind value");
        }
        const sourceId = entry.sourceId;
        const input = inputs(name).get(sourceId);
        if (!input) {
          this.error(`No Stillwind producer for ${sourceId}`);
        }
        const watchSource = settings.watching
          ? (file: string) => settings.devServerRef?.watcher.add(cleanModuleId(file))
          : this.addWatchFile.bind(this);
        watchSource(sourceId);
        const plan = await project
          .environment(name)
          .modulePlan([input.moduleId], this, !settings.watching);
        for (const producer of plan.producers) {
          watchSource(resolve(settings.projectRoot, cleanModuleId(producer.module)));
        }
        for (const file of project.environment(name).designSystem.cssFiles) {
          this.addWatchFile(file);
        }
        const imports = plan.rawParts.map(
          (part, index) =>
            `import { css as p${index} } from ${JSON.stringify(
              registry.fragment(
                settings.watching && settings.devSourceMaps
                  ? cssForBrowser(name, part, new Map())
                  : part.css,
                plan.resources,
              ),
            )};\n`,
        );
        return [
          "// Stillwind CSS value (vite)\n",
          ...imports,
          "\n",
          "export const parts = [",
          ...plan.parts.map(
            (part, index) => `  { key: ${JSON.stringify(part.key)}, css: p${index} },\n`,
          ),
          "];\n",
          'export const css = parts.map((part) => part.css).join("");\n',
          "export default css;\n",
        ].join("");
      }
      return undefined;
    },
  };
}
