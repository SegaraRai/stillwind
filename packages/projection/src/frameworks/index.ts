import { createHash } from "node:crypto";
import { createRequire } from "node:module";
import { extname } from "node:path";

import type { ProjectSource, ProjectionMap } from "../protocol.ts";

export interface ScriptRegion {
  id: number;
  start: number;
  end: number;
}
export interface ConvertedFramework {
  source: string;
  language: "ts" | "tsx";
  framework: NonNullable<ProjectSource["framework"]>;
  fw2js_map: ProjectionMap;
  converter: NonNullable<ProjectSource["converter"]>;
  import_realms: ScriptRegion[];
  regions: NonNullable<ProjectSource["regions"]>;
}

const require = createRequire(import.meta.url);
export function converterIdentity(name: string, options: unknown) {
  const metadata: unknown = require(`${name}/package.json`);
  if (
    !metadata ||
    typeof metadata !== "object" ||
    !("version" in metadata) ||
    typeof metadata.version !== "string"
  ) {
    throw new Error(`Missing converter version for ${name}`);
  }
  return {
    name,
    version: metadata.version,
    options_fingerprint: createHash("sha256").update(JSON.stringify(options)).digest("hex"),
  };
}

/** Partition original UTF-16 positions without overlapping script/template regions. */
export function sourceRegions(
  source: string,
  scripts: ScriptRegion[],
  templateRealm: number | undefined,
) {
  const regions: NonNullable<ProjectSource["regions"]> = [];
  let cursor = 0;
  for (const script of [...scripts].sort((a, b) => a.start - b.start)) {
    if (
      !Number.isInteger(script.start) ||
      !Number.isInteger(script.end) ||
      script.start < cursor ||
      script.end < script.start ||
      script.end > source.length
    ) {
      throw new Error("Converter returned invalid or overlapping script regions");
    }
    if (templateRealm !== undefined && cursor < script.start)
      regions.push({ start: cursor, end: script.start, import_realm: templateRealm });
    if (script.start < script.end)
      regions.push({ start: script.start, end: script.end, import_realm: script.id });
    cursor = script.end;
  }
  if (templateRealm !== undefined && cursor < source.length)
    regions.push({ start: cursor, end: source.length, import_realm: templateRealm });
  return regions;
}

export function sourceMapV3(value: string | object): ProjectionMap {
  const json = typeof value === "string" ? value : JSON.stringify(value);
  const parsed: unknown = JSON.parse(json);
  if (
    !parsed ||
    typeof parsed !== "object" ||
    !("sources" in parsed) ||
    !Array.isArray(parsed.sources) ||
    parsed.sources.length !== 1 ||
    typeof parsed.sources[0] !== "string"
  ) {
    throw new Error("Framework projection requires one original source in its map");
  }
  return { kind: "v3", json, source: parsed.sources[0] };
}

/** Project raw SFC input while retaining the original buffer as the native edit target. */
export async function projectFrameworkSource(input: ProjectSource): Promise<ProjectSource> {
  if (input.fw_code !== undefined) return input;
  let converted: ConvertedFramework;
  switch (extname(input.path).toLowerCase()) {
    case ".vue":
      converted = (await import("./vue.ts")).convert(input.source, input.path);
      break;
    case ".svelte":
      converted = (await import("./svelte.ts")).convert(input.source, input.path);
      break;
    case ".astro":
      converted = await (await import("./astro.ts")).convert(input.source, input.path);
      break;
    default:
      return input;
  }
  if (input.framework !== undefined && input.framework !== converted.framework)
    throw new Error(
      `Framework ${input.framework} conflicts with ${converted.framework} source ${input.path}`,
    );
  return { ...input, ...converted, fw_code: input.source, fw_map: input.map, map: undefined };
}
