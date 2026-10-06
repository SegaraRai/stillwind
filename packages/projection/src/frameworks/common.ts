import { createHash } from "node:crypto";
import { createRequire } from "node:module";

import { PreparationError, originalRange, type ScriptRegion } from "../preparation.ts";
import type { ConverterIdentity, ProjectionMap, SourceProjection } from "../protocol.ts";

const require = createRequire(import.meta.url);

/**
 * Reads a converter's package version and hashes its serialized configuration.
 *
 * @param name - Package name resolvable from this module with an accessible `package.json`.
 * @param options - Configuration that `JSON.stringify` can serialize to a string. The hash depends
 * on serialized property order.
 * @returns The package name, installed version, and SHA-256 hex digest of the serialized options.
 * @throws If package metadata cannot be loaded, its version is missing or not a string, or the
 * configuration cannot be serialized and hashed.
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
 * Partitions original text into editable ranges with the appropriate import realm.
 *
 * @param source - Original component text used to validate UTF-16 boundaries.
 * @param scripts - Nonoverlapping script bodies with realm identifiers. Input order may vary.
 * @param templateRealm - Realm for gaps between scripts and the remaining template text. Undefined
 * omits these gaps from the result.
 * @param converter - Optional converter identity attached to a projection failure.
 * @returns Sorted, nonempty ranges. Scripts use their own realm, and gaps use `templateRealm` when
 * supplied. The input array and script records are not mutated.
 * @throws {PreparationError} If script bounds are invalid, split a surrogate pair, or overlap.
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
 * Wraps a converter map after checking that it identifies exactly one original source.
 *
 * @param value - Serialized source-map JSON or a serializable map object.
 * @param converter - Optional converter identity attached to a projection failure.
 * @returns A version 3 projection descriptor containing the JSON and its sole source name. Other
 * source-map fields are validated when the map is consumed.
 * @throws {PreparationError} If JSON is invalid or `sources` does not contain exactly one string.
 * @throws If the supplied object cannot be serialized.
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
