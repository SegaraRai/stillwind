import { createHash } from "node:crypto";
import { createRequire } from "node:module";

import type { ConverterIdentity, ProjectionMap, SourceProjection } from "../protocol.ts";

/** A recognized component syntax or projection failure that the host can report as a diagnostic. */
export class PreparationError extends Error {
  /** Failure category and the message exposed to the host. */
  readonly issue: {
    /** `syntax` identifies rejected component syntax. `projection` identifies unusable conversion. */
    kind: "syntax" | "projection";
    /** Diagnostic text also used as this error's message. */
    detail: string;
  };
  /** Converter that produced the failure, when its identity is available. */
  readonly converter?: ConverterIdentity;
  /** Validated original range for a diagnostic. Omission makes the diagnostic document-wide. */
  readonly range?: {
    /** Inclusive UTF-16 offset in the original document. */
    start: number;
    /** Exclusive UTF-16 offset in the original document, at or after `start`. */
    end: number;
  };

  /**
   * Records a preparation failure without changing the supplied metadata.
   *
   * @param issue - Failure category and diagnostic text. `detail` becomes the error message.
   * @param converter - Converter identity, when known.
   * @param range - Optional original UTF-16 range already validated by `originalRange` or the caller.
   */
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

/**
 * Validates diagnostic bounds against the original text, including surrogate pair boundaries.
 *
 * @param source - Original document text.
 * @param start - Inclusive UTF-16 offset, or an omitted parser position.
 * @param end - Exclusive UTF-16 offset, or an omitted parser position.
 * @returns The bounds, including an empty range, or undefined for missing, noninteger, reversed,
 * out-of-bounds positions or positions inside a surrogate pair.
 */
export function originalRange(source: string, start?: number, end?: number) {
  if (
    start == null ||
    end == null ||
    !Number.isSafeInteger(start) ||
    !Number.isSafeInteger(end) ||
    start < 0 ||
    end < start ||
    end > source.length
  ) {
    return undefined;
  }

  const boundary = (offset: number) =>
    offset === 0 ||
    offset === source.length ||
    !(
      source.charCodeAt(offset - 1) >= 0xd800 &&
      source.charCodeAt(offset - 1) <= 0xdbff &&
      source.charCodeAt(offset) >= 0xdc00 &&
      source.charCodeAt(offset) <= 0xdfff
    );
  if (!boundary(start) || !boundary(end)) {
    return undefined;
  }

  return {
    start,
    end,
  };
}

/** Script body in the original component that defines a scope for imports. */
export interface ScriptRegion {
  /** Realm identifier used by editable regions. Must be unique within the component. */
  id: number;
  /** Inclusive UTF-16 offset in the original document, at a complete character boundary. */
  start: number;
  /** Exclusive UTF-16 offset, at or after `start` and at a complete character boundary. */
  end: number;
  /** Parent realm identifier whose imports this script can use, when present. */
  parent?: number;
}

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
