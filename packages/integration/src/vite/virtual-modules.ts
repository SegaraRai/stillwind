import { readFile } from "node:fs/promises";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";

import { cleanModuleId, cssWithSourceMap, decodeCssSourceMap } from "@stillwind/host";
import { transform as transformCss } from "esbuild";
import type { Plugin } from "vite";

import {
  runtimeId,
  valuePrefix,
  fragmentPrefix,
  projectionPrefix,
  producerPrefix,
  runtimeSpecifier,
  virtualCssId,
} from "./module-ids.ts";
import type { ViteProject } from "./project.ts";

const runtimeEntry = "virtual:stillwind/runtime-entry";
const sourceMapEntry = "virtual:stillwind/runtime-source-map";

function runtimeFile(file: string) {
  return fileURLToPath(new URL(`./${file}.js`, import.meta.resolve("@stillwind/integration/vite")));
}

export function virtualModuleHooks(project: ViteProject): Pick<Plugin, "resolveId" | "load"> {
  const {
    settings,
    environmentState,
    inputMap,
    includeSourceContent,
    fragments,
    fragmentSpecifier,
    producerSpecifier,
    resolvePlan,
    documentPlan,
    publish,
    compile,
  } = project;
  return {
    async resolveId(id, importer) {
      if (id === runtimeEntry) {
        return runtimeFile("runtime");
      }
      if (id === sourceMapEntry) {
        return runtimeFile("runtime-source-map");
      }
      if (id.endsWith("?stillwind.css")) {
        // CSS projections do not expose server JavaScript to the client graph.
        const source = await this.resolve(id.slice(0, -"?stillwind.css".length), importer, {
          skipSelf: true,
          custom: {
            "react-router:dot-server": true,
          },
        });
        if (!source || source.external) {
          this.error(`Cannot resolve Stillwind projection ${id}`);
        }
        return projectionPrefix + Buffer.from(cleanModuleId(source.id)).toString("base64url");
      }
      if (id === runtimeSpecifier || id === virtualCssId) {
        return runtimeId;
      }
      if (id === "virtual:stillwind/snapshot.css") {
        return "\0stillwind:snapshot.css";
      }
      if (id.startsWith("virtual:stillwind/value:")) {
        return valuePrefix + id.slice("virtual:stillwind/value:".length);
      }
      if (id.startsWith("virtual:stillwind/fragment:")) {
        return fragmentPrefix + id.slice("virtual:stillwind/fragment:".length);
      }
      if (id.startsWith("virtual:stillwind/producer:")) {
        return producerPrefix + id.slice("virtual:stillwind/producer:".length);
      }
      if (id.startsWith("\0stillwind:")) {
        return id;
      }
      return undefined;
    },
    async load(id) {
      const name = this.environment?.name;
      if (id === "\0stillwind:snapshot.css") {
        const plan = await documentPlan(name, this);
        return decodeCssSourceMap(
          cssWithSourceMap(
            { css: plan.originalCss, sourceMap: plan.sourceMap },
            plan.resourceReplacements,
            [...inputMap(name).values()].flatMap((input) =>
              input.upstream ? [input.upstream] : [],
            ),
            includeSourceContent(name),
          ),
        );
      }
      if (id.startsWith(producerPrefix)) {
        if (!settings.watching) {
          return "// Stillwind producer (vite)\nexport {};\n";
        }
        const producer = Buffer.from(id.slice(producerPrefix.length), "base64url").toString("utf8");
        return `// Stillwind producer (vite)
import { register, synchronize, unregister } from ${JSON.stringify(runtimeSpecifier)};

await synchronize(${environmentState(name).revision});
register(${JSON.stringify(producer)});
if (import.meta.hot) {
  import.meta.hot.prune(() => {
    unregister(${JSON.stringify(producer)});
  });
}
`;
      }
      if (id.startsWith(projectionPrefix)) {
        const source = Buffer.from(id.slice(projectionPrefix.length), "base64url").toString("utf8");
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
        const result = await compile(name, await readFile(source, "utf8"), source, context);
        const input = inputMap(name).get(source);
        if (input) {
          input.manual = false;
          input.projection = true;
        }
        await publish(name, context);
        return {
          code: `// Stillwind projection\nimport ${JSON.stringify(producerSpecifier(source))};\n`,
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
        const plan = await documentPlan(name, this);
        const revision = environmentState(name).revision;
        const snapshot = {
          revision,
          parts: plan.parts,
          dependencies: Object.fromEntries(
            [...inputMap(name).values()].map((input) => [input.moduleId, input.dependencies]),
          ),
        };
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
        const key = id.slice(fragmentPrefix.length);
        let css = settings.watching
          ? fragments.get(key)
          : Buffer.from(key, "base64url").toString("utf8");
        if (css === undefined) {
          this.error(`Unknown Stillwind CSS fragment ${key}`);
        }
        if (!settings.watching && settings.minifyCss) {
          css = (await transformCss(css, { loader: "css", minify: true })).code;
        }
        const files = [
          ...new Set([...css.matchAll(/\/@fs\/([^"'\s)]+)/g)].map((match) => match[1])),
        ];
        const imports = files.map(
          (file, index) =>
            `import asset${index} from ${JSON.stringify(decodeURIComponent(file) + "?url")};\n`,
        );
        const replacements = files
          .map(
            (file, index) =>
              `.replaceAll(${JSON.stringify(`/@fs/${file}`)}, JSON.stringify(asset${index}).slice(1, -1))`,
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
        const sourceId = Buffer.from(id.slice(valuePrefix.length), "base64url").toString("utf8");
        this.addWatchFile(cleanModuleId(sourceId));
        if (!inputMap(name).has(sourceId)) {
          await compile(name, await readFile(cleanModuleId(sourceId), "utf8"), sourceId, this);
        }
        const input = inputMap(name).get(sourceId);
        if (!input) {
          this.error(`No Stillwind producer for ${sourceId}`);
        }
        const plan = await resolvePlan(
          environmentState(name),
          [input.moduleId],
          this,
          !settings.watching,
        );
        for (const producer of plan.producers) {
          this.addWatchFile(resolve(settings.projectRoot, cleanModuleId(producer.module)));
        }
        for (const file of environmentState(name).cssFiles) {
          this.addWatchFile(file);
        }
        const imports = plan.parts.map(
          (part, index) =>
            `import { css as p${index} } from ${JSON.stringify(fragmentSpecifier(part.css))};\n`,
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
