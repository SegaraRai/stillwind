import { createHash } from "node:crypto";
import { createRequire } from "node:module";

import type { ConverterIdentity, ProjectionMap, SourceProjection } from "../protocol.ts";

/**
 * An original-buffer script realm with half-open UTF-16 bounds. parent is a realm id.
 */
export interface ScriptRegion {
  id: number;
  start: number;
  end: number;
  parent?: number;
}

const require = createRequire(import.meta.url);

/**
 * Reads the converter package version and hashes JSON.stringify(options) with SHA-256.
 * The fingerprint depends on serialized property order, not semantic object equality.
 * Throws if package metadata cannot be loaded, version is missing, or options cannot
 * be serialized to a JSON string.
 */
export function converterIdentity(name: string, options: unknown): ConverterIdentity {
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
    optionsFingerprint: createHash("sha256").update(JSON.stringify(options)).digest("hex"),
  };
}

/**
 * Partitions original UTF-16 positions into sorted, nonoverlapping regions without
 * mutating scripts. Nonempty scripts use their own realm; gaps use templateRealm
 * when supplied and are otherwise omitted. Throws for noninteger, out-of-bounds,
 * reversed, or overlapping script ranges.
 */
export function sourceRegions(
  source: string,
  scripts: ScriptRegion[],
  templateRealm: number | undefined,
) {
  const regions: SourceProjection["editableRegions"] = [];
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
    if (templateRealm !== undefined && cursor < script.start) {
      regions.push({
        start: cursor,
        end: script.start,
        importRealm: templateRealm,
      });
    }
    if (script.start < script.end) {
      regions.push({
        start: script.start,
        end: script.end,
        importRealm: script.id,
      });
    }
    cursor = script.end;
  }
  if (templateRealm !== undefined && cursor < source.length) {
    regions.push({
      start: cursor,
      end: source.length,
      importRealm: templateRealm,
    });
  }
  return regions;
}

/**
 * Wraps a serialized map or map object as a v3 projection map. Requires exactly one
 * string source name; this check does not validate every source-map field.
 * Throws for serialization/JSON errors or an unsupported sources array.
 */
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
  return {
    kind: "v3",
    json,
    source: parsed.sources[0],
  };
}
