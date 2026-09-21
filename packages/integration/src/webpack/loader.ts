import { createHash } from "node:crypto";
import fs from "node:fs";
import { isBuiltin } from "node:module";
import path from "node:path";
import { fileURLToPath } from "node:url";

import type { LoaderContext } from "webpack";

import {
  compilerOptions,
  cssEntrypoint,
  writeVocabulary,
  type StillwindOptions,
} from "../options.ts";
import { IntegrationSession } from "../session.ts";
import { registerEmission } from "./emission.ts";
import { loaderOptionsId } from "./rules.ts";

const fingerprint = (source: string) => createHash("sha256").update(source).digest("hex");

const relativeRequest = (from: string, to: string) => {
  const value = path.relative(from, to).replaceAll("\\", "/");
  return value.startsWith(".") ? value : `./${value}`;
};

export default function (
  this: LoaderContext<StillwindOptions & { stillwindOptions?: string }>,
  source: string,
  inputMap: any,
  additionalData: any,
) {
  const callback = this.async();
  const text = String(source);
  const prior = additionalData?.stillwind?.sourceTransform;
  // Framework loaders can clone the same rule into a subrequest's chain.
  // Skip only an identical output already produced for this resource.
  if (prior?.resource === this.resourcePath && prior.fingerprint === fingerprint(text)) {
    return callback(null, source, inputMap, additionalData);
  }
  const projection = /[?&]stillwind\.css(?:&|$)/.test(this.resourceQuery ?? "");
  const cssValue = /[?&]stillwind-module-css(?:&|$)/.test(this.resourceQuery ?? "");
  const sfc = /\.(vue|svelte|astro)$/.test(this.resourcePath);
  if (/[?&](?:raw|url)(?:[=&]|$)/.test(this.resourceQuery ?? "")) {
    return callback(null, source, inputMap);
  }
  if (
    !projection &&
    !cssValue &&
    !/stillwind\/(?:react|preact|solid|vue|svelte|astro|module-css)/.test(text)
  ) {
    return callback(null, source, inputMap);
  }
  const suppliedOptions = this.getOptions();
  const options: StillwindOptions = suppliedOptions.stillwindOptions
    ? JSON.parse(Buffer.from(suppliedOptions.stillwindOptions, "base64url").toString("utf8"))
    : suppliedOptions;
  if (!options.css && !options.theme) {
    return callback(new Error("Stillwind requires a CSS entrypoint"));
  }
  if (options.naming) {
    this.cacheable(false);
  }
  let compiler: IntegrationSession | undefined;
  // Loader requests remain reconstructible in another worker. Their payload
  // contains configuration, not generated CSS or mutable process-local state.
  const virtualRequest = (query: string) =>
    sfc
      ? `!!${import.meta.filename.replaceAll("\\", "/")}${options.naming ? "??" + loaderOptionsId(options) : "?stillwindOptions=" + Buffer.from(JSON.stringify(options)).toString("base64url")}!${this.resourcePath.replaceAll("\\", "/")}?${query}`
      : `${relativeRequest(path.dirname(this.resourcePath), this.resourcePath)}?${query}`;
  const read = (id: string) =>
    new Promise<string>((resolve, reject) =>
      this.fs.readFile(id, (error, value) => (error ? reject(error) : resolve(String(value)))),
    );
  const resolver = (settings: Parameters<LoaderContext<StillwindOptions>["getResolve"]>[0]) => {
    const resolve = this.getResolve(settings);
    return (specifier: string, importer: string) =>
      new Promise<string>((done, reject) =>
        resolve(path.dirname(importer), specifier, (error, id) =>
          error ? reject(error) : done(id || ""),
        ),
      );
  };
  const resolveCss = resolver({
    conditionNames: ["style", "import", "default"],
    extensions: [".css"],
    mainFields: ["style", "main"],
  });
  const resolveJs = resolver({
    dependencyType: "esm",
    extensions: [".tsx", ".ts", ".jsx", ".js", ".mjs", ".cjs", ".json"],
  });
  const resolveAsset = resolver({});
  const watch = (id: string) => this.addDependency(id);
  watch(import.meta.filename);
  void (async () => {
    const host = await import("@stillwind/host");
    const { nativeBindingPath } = await import("@stillwind/native");
    watch(nativeBindingPath);
    watch(fileURLToPath(import.meta.resolve("@stillwind/native")));
    const hostDirectory = path.dirname(fileURLToPath(import.meta.resolve("@stillwind/host")));
    for (const file of fs.readdirSync(hostDirectory)) {
      if (file.endsWith(".js")) {
        watch(path.join(hostDirectory, file));
      }
    }
    const root = path.resolve(this.rootContext, options.root ?? ".");
    const css = path.resolve(root, cssEntrypoint(options));
    const loaded = await host.loadCssGraph(css, compilerOptions(options), {
      resolve: resolveCss,
      read,
      watch,
    });
    writeVocabulary(loaded.loader, root, options);
    compiler = new IntegrationSession(
      root,
      loaded.loader,
      options,
      false,
      this.mode === "development" ? "development" : "release",
    );
    const session = compiler.native;
    const output = await compiler.compile(
      projection || cssValue ? await read(this.resourcePath) : text,
      this.resourcePath,
      {
        resolve: async (specifier, importer) => {
          if (specifier.startsWith("node:") && isBuiltin(specifier)) {
            return { id: specifier, external: true };
          }
          try {
            const id = await resolveJs(specifier, importer);
            return id ? { id } : null;
          } catch (error) {
            if (isBuiltin(specifier)) {
              return { id: specifier, external: true };
            }
            throw error;
          }
        },
        read,
        watch,
        warn: (message) => this.emitWarning(new Error(message)),
      },
      {
        stylesheetImports:
          sfc && !projection && !cssValue && options.automatic !== false
            ? [virtualRequest("stillwind.css")]
            : undefined,
        moduleCssImport: virtualRequest("stillwind-module-css"),
      },
    );
    if (!output?.code) {
      return callback(null, projection ? "" : source, projection ? null : inputMap);
    }
    const plan = session.planStylesheet({
      modules: [output.path],
      coverage: { kind: "observed" },
      placement: "document",
    });

    const valueResources: { placeholder: string; id: string; request: string }[] = [];
    for (const resource of [...loaded.resources, ...plan.resources]) {
      const resolved = await resolveAsset(resource.specifier, path.resolve(root, resource.owner));
      if (!resolved) {
        throw new Error(
          `Cannot resolve Stillwind resource ${resource.specifier} from ${resource.owner}`,
        );
      }
      watch(host.cleanModuleId(resolved));
      if (plan.css.includes(resource.placeholder)) {
        valueResources.push({
          placeholder: resource.placeholder,
          id: resolved,
          request: relativeRequest(path.dirname(this.resourcePath), resolved),
        });
      }
    }
    const carrier = relativeRequest(
      path.dirname(this.resourcePath),
      fileURLToPath(
        new URL("./stillwind-webpack.css", import.meta.resolve("@stillwind/integration/webpack")),
      ),
    );
    if (cssValue) {
      const imports = plan.parts
        .map((part, index) => {
          const payload = Buffer.from(
            JSON.stringify({
              css: part.css,
              resources: valueResources.filter((resource) =>
                part.css.includes(resource.placeholder),
              ),
            }),
          ).toString("base64url");
          const request = `!!${fileURLToPath(
            new URL("./fragment-loader.js", import.meta.resolve("@stillwind/integration/webpack")),
          )}!${fileURLToPath(
            new URL(
              "./stillwind-fragment.js",
              import.meta.resolve("@stillwind/integration/webpack"),
            ),
          )}?css=${payload}`;
          return `import {css as p${index}} from ${JSON.stringify(request)};`;
        })
        .join("\n");
      const parts = plan.parts
        .map((part, index) => `{key:${JSON.stringify(part.key)},css:p${index}}`)
        .join(",");
      return callback(
        null,
        `${imports}\nexport const parts=[${parts}];\nexport const css=parts.map(part=>part.css).join("");\nexport default css;`,
        null,
      );
    }
    const automatic = (!output.hasModuleCssImport && options.automatic !== false) || projection;
    if (automatic) {
      registerEmission(this, {
        source: text,
        id: this.resourcePath,
        root,
        options,
        manual: false,
      });
    }
    const cssImport = `import ${JSON.stringify(carrier + "?stillwind-document")};\n`;
    if (projection) {
      return callback(null, cssImport);
    }
    const finalized = host.finalizeModuleWithMap(output.code);
    // Append synthetic imports so RSC directives remain the first statements.
    const code =
      sfc || !automatic
        ? finalized.code
        : finalized.code.slice(finalized.prefix.length) + "\n" + finalized.prefix + cssImport;
    const map = host.moduleSourceMap(
      text,
      output.code,
      this.resourcePath,
      output.mappings,
      finalized.removed,
      "",
    );
    const composedMap = host.composeModuleSourceMap(map, inputMap);
    if (sfc && this.data) {
      (this.data as any).stillwindSourceTransform = {
        code,
        map: composedMap,
        resource: this.resourcePath,
      };
    }
    callback(null, code, composedMap as Parameters<typeof callback>[2], {
      ...additionalData,
      stillwind: {
        coverage: plan.coverage,
        designSystem: plan.designSystem,
        producers: plan.producers,
        sourceTransform: { resource: this.resourcePath, fingerprint: fingerprint(code) },
      },
    });
  })()
    .catch((error) => callback(error))
    .finally(() => compiler?.close());
}

// Allocate loader-chain-local data for the post-framework map bridge.
export function pitch() {}
