import { createHash } from "node:crypto";
import { createRequire } from "node:module";

import type { ConverterIdentity, ProjectionMap, SourceProjection } from "../protocol.ts";

/** A known failure in a component parser or its projection. */
export class PreparationError extends Error {
  readonly issue: {
    kind: "syntax" | "projection";
    detail: string;
  };
  readonly converter?: ConverterIdentity;
  readonly range?: {
    start: number;
    end: number;
  };

  constructor(
    issue: PreparationError["issue"],
    converter?: ConverterIdentity,
    range?: PreparationError["range"],
  ) {
    super(issue.detail);
    this.issue = issue;
    this.converter = converter;
    this.range = range;
  }
}

/** Returns valid original UTF-16 bounds, or no range when the parser has no usable position. */
export function originalRange(source: string, start?: number, end?: number) {
  const boundary = (offset: number) =>
    offset === 0 ||
    offset === source.length ||
    !(
      source.charCodeAt(offset - 1) >= 0xd800 &&
      source.charCodeAt(offset - 1) <= 0xdbff &&
      source.charCodeAt(offset) >= 0xdc00 &&
      source.charCodeAt(offset) <= 0xdfff
    );
  return start !== undefined &&
    end !== undefined &&
    Number.isSafeInteger(start) &&
    Number.isSafeInteger(end) &&
    start >= 0 &&
    end >= start &&
    end <= source.length &&
    boundary(start) &&
    boundary(end)
    ? { start, end }
    : undefined;
}

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
  converter?: ConverterIdentity,
) {
  const regions: SourceProjection["editableRegions"] = [];
  let cursor = 0;
  for (const script of [...scripts].sort((a, b) => a.start - b.start)) {
    if (!originalRange(source, script.start, script.end) || script.start < cursor) {
      throw new PreparationError(
        {
          kind: "projection",
          detail: "Converter returned invalid or overlapping script regions",
        },
        converter,
      );
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
export function sourceMapV3(value: string | object, converter?: ConverterIdentity): ProjectionMap {
  const json = typeof value === "string" ? value : JSON.stringify(value);
  let parsed: unknown;
  try {
    parsed = JSON.parse(json);
  } catch (error) {
    if (!(error instanceof SyntaxError)) {
      throw error;
    }
    throw new PreparationError(
      {
        kind: "projection",
        detail: "Converter returned invalid source-map JSON",
      },
      converter,
    );
  }
  if (
    !parsed ||
    typeof parsed !== "object" ||
    !("sources" in parsed) ||
    !Array.isArray(parsed.sources) ||
    parsed.sources.length !== 1 ||
    typeof parsed.sources[0] !== "string"
  ) {
    throw new PreparationError(
      {
        kind: "projection",
        detail: "Framework projection requires one original source in its map",
      },
      converter,
    );
  }
  return {
    kind: "v3",
    json,
    source: parsed.sources[0],
  };
}
