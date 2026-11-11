import { createHash } from "node:crypto";
import path from "node:path";
import { fileURLToPath } from "node:url";

import {
  cleanModuleId,
  cssWithSourceMap,
  warnRejectedNames,
  type CssUpstreamMap,
} from "@stillwind/host";

import { warnSkippedCss } from "../session.ts";
import type { ProducerInput } from "./emission.ts";
import type { CompilationHost } from "./host.ts";
import type { SharedProject } from "./project.ts";
import { relativeRequest } from "./requests.ts";
import { sourceDescriptor } from "./sources.ts";

/**
 * Renders document CSS for a completed compilation from the producers that it recorded.
 *
 * A producer is compiled again only when the shared compiler holds no loader compilation of its
 * current source. Manual producers are compiled for dependency composition but are excluded from
 * the document selection.
 *
 * @param producers - Nonempty list of producers from the completed graph. Every producer must use
 * the same root and config.
 * @param io - Compilation host that reads captured dependency sources and resolves scripts and
 * assets.
 * @param project - Shared project of the producers' design system, which no producer loader uses
 * meanwhile.
 * @param watch - Registers a file as a compilation dependency.
 * @param warn - Reports compiler warnings and the CSS statements that loading skipped through the
 * bundler.
 * @returns Document CSS with asset requests and an inline source map composed with recorded
 * upstream maps.
 * @throws If producers use different design systems, a dependency source is missing, or
 * compilation fails.
 */
export async function compilationStyles(
  producers: readonly ProducerInput[],
  io: CompilationHost,
  project: SharedProject,
  watch: (id: string) => void,
  warn: (message: string) => void,
): Promise<string> {
  const { config, root } = producers[0];
  const { compiler, loaded, outputs } = project;
  const readDependency = io.dependencyReader(producers);
  warnSkippedCss(loaded.skipped, warn);
  const selection: string[] = [];
  for (const input of producers) {
    if (input.root !== root || JSON.stringify(input.config) !== JSON.stringify(config)) {
      throw new Error("A compilation must use one Stillwind design system");
    }
    const previous = outputs.get(input.id);
    if (previous?.source === input.source) {
      if (previous.moduleId !== undefined && !input.manual) {
        selection.push(previous.moduleId);
      }
      continue;
    }
    const output = await compiler.compile(
      input.source,
      input.id,
      {
        describe: (id) => sourceDescriptor(root, id),
        resolve: async (specifier, importer) => ({
          id: await io.resolveJs(specifier, importer),
        }),
        read: readDependency,
        watch: (id) => {
          watch(cleanModuleId(id));
        },
        warn,
      },
      {
        moduleCssImport: "stillwind/module-css",
      },
    );
    outputs.set(input.id, { source: input.source, moduleId: output?.moduleId });
    if (output && !input.manual) {
      selection.push(output.moduleId);
    }
  }
  const revision = createHash("sha256")
    .update(producers.map((input) => `${input.source}\0`).join(""))
    .digest("hex");
  const plan = compiler.native.planStylesheet({
    modules: selection,
    coverage: {
      kind: "complete",
      target: "compilation",
      revision,
    },
    placement: "document",
  });
  warnRejectedNames(plan.rejectedNames, warn);
  const replacements = new Map<string, string>();
  for (const resource of [...loaded.resources, ...plan.resources]) {
    const owner =
      "moduleId" in resource ? compiler.resolverId(resource.moduleId) : resource.sourceName;
    const file = await io.resolveAsset(resource.specifier, owner);
    watch(file);
    const request = relativeRequest(
      path.dirname(fileURLToPath(import.meta.resolve("@stillwind/integration/webpack"))),
      file,
    );
    replacements.set(resource.placeholder, JSON.stringify(request).slice(1, -1));
  }
  return cssWithSourceMap(
    plan,
    replacements,
    producers.flatMap((input) =>
      [
        input.upstream,
        ...(input.dependencies ?? []).map((dependency) => dependency.upstream),
      ].filter((upstream): upstream is CssUpstreamMap => Boolean(upstream)),
    ),
  );
}
