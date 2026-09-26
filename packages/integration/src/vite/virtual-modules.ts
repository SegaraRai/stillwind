import { readFile } from "node:fs/promises";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

import { cleanModuleId, cssWithSourceMap, decodeCssSourceMap } from "@stillwind/host";
import { build, transform as transformCss } from "esbuild";
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
import { developmentRuntime } from "./runtime.ts";

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
      if (id.endsWith("?stillwind.css")) {
        const source = await this.resolve(id.slice(0, -"?stillwind.css".length), importer, {
          skipSelf: true,
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
          return "export {};";
        }
        const producer = Buffer.from(id.slice(producerPrefix.length), "base64url").toString("utf8");
        return `import {register,unregister,synchronize} from ${JSON.stringify(runtimeSpecifier)}; await synchronize(${environmentState(name).revision}); register(${JSON.stringify(producer)}); if(import.meta.hot) import.meta.hot.prune(()=>unregister(${JSON.stringify(producer)}));`;
      }
      if (id.startsWith(projectionPrefix)) {
        const source = Buffer.from(id.slice(projectionPrefix.length), "base64url").toString("utf8");
        this.addWatchFile(source);
        const result = await compile(name, await readFile(source, "utf8"), source, this);
        const input = inputMap(name).get(source);
        if (input) {
          input.manual = false;
          input.projection = true;
        }
        await publish(name, this);
        return {
          code: `import ${JSON.stringify(producerSpecifier(source))};`,
          meta: { stillwind: { ...result?.meta.stillwind, projection: true } },
        };
      }
      if (id === runtimeId) {
        if (!settings.watching) {
          return "export {};";
        }
        const plan = await documentPlan(name, this);
        const revision = environmentState(name).revision;
        const code = developmentRuntime(
          {
            revision,
            parts: plan.parts,
            dependencies: Object.fromEntries(
              [...inputMap(name).values()].map((input) => [input.path, input.dependencies]),
            ),
          },
          settings.devSourceMaps,
          `${settings.publicBase}__stillwind/css?environment=${encodeURIComponent(name ?? "client")}`,
        );
        if (!settings.devSourceMaps) {
          return code;
        }
        const bundled = await build({
          stdin: {
            contents: code,
            resolveDir: dirname(fileURLToPath(import.meta.url)),
          },
          bundle: true,
          write: false,
          format: "esm",
          platform: "browser",
        });
        return bundled.outputFiles[0].text;
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
        const imports = files
          .map(
            (file, index) =>
              `import asset${index} from ${JSON.stringify(decodeURIComponent(file) + "?url")};`,
          )
          .join("\n");
        const replacements = files
          .map(
            (file, index) =>
              `.replaceAll(${JSON.stringify(`/@fs/${file}`)}, JSON.stringify(asset${index}).slice(1,-1))`,
          )
          .join("");
        return `${imports}\nexport const css = ${JSON.stringify(css)}${replacements};`;
      }
      if (id.startsWith(valuePrefix)) {
        const sourceId = Buffer.from(id.slice(valuePrefix.length), "base64url").toString("utf8");
        this.addWatchFile(sourceId);
        if (!inputMap(name).has(sourceId)) {
          await compile(name, await readFile(sourceId, "utf8"), sourceId, this);
        }
        const input = inputMap(name).get(sourceId);
        if (!input) {
          this.error(`No Stillwind producer for ${sourceId}`);
        }
        const plan = await resolvePlan(
          environmentState(name),
          [input.path],
          this,
          !settings.watching,
        );
        for (const producer of plan.producers) {
          this.addWatchFile(resolve(settings.projectRoot, producer.module));
        }
        for (const file of environmentState(name).cssFiles) {
          this.addWatchFile(file);
        }
        const imports = plan.parts
          .map(
            (part, index) =>
              `import { css as p${index} } from ${JSON.stringify(fragmentSpecifier(part.css))};`,
          )
          .join("\n");
        return `${imports}\nexport const parts = [${plan.parts.map((part, index) => `{key:${JSON.stringify(part.key)},css:p${index}}`).join(",")}];\nexport const css = parts.map(part => part.css).join("");\nexport default css;`;
      }
      return undefined;
    },
  };
}
