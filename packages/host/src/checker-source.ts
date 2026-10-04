import path from "node:path";

import { TraceMap, decodedMappings } from "@jridgewell/trace-mapping";
import type { NativeAssetRecord, NativeCheckerBinding } from "@stillwind/native";
import type { PreparedSource, CheckerBuffer } from "@stillwind/projection";
import { skipTrivia } from "typescript/unstable/ast";

/** Original UTF-8 expression range and its validated projected UTF-16 range. */
export interface CheckerSpan {
  role: string;
  start: number;
  end: number;
  projectedStart: number;
  projectedEnd: number;
}

/** Original-coordinate requests for one module and its optional parser projection. */
export type TypeFactSource = PreparedSource & {
  /**
   * Half-open UTF-8 ranges in document.code.
   * When `recognitionOnly` is true, the query only checks whether the receiver is
   * a Stillwind macro asset. A type that cannot be resolved produces no fact.
   * When false or omitted, the query follows the required-query policy for lowering.
   * Every probe must still match an expression in the checker AST.
   */
  probes: readonly { start: number; end: number; recognitionOnly?: boolean }[];
  /** Validated original-byte to projected-UTF-16 correspondences from the frontend. */
  checkerSpans?: readonly CheckerSpan[];
  /** Validated generated references that must use their global helper declarations. */
  checkerBindings?: readonly NativeCheckerBinding[];
  /** Alternative checker buffers, including independent Astro client modules. */
  checkerBuffers?: readonly CheckerBuffer[];
  /** Absolute framework declaration files included in the checker project. */
  checkerTypes?: readonly string[];
  /** Original UTF-8 string-literal ranges and resolved original module ids. */
  resolutions?: readonly { start: number; end: number; id: string }[];
};

/** Checker-only insertion/replacement in input-buffer UTF-16 coordinates. */
interface ShadowEdit {
  at: number;
  end: number;
  text: string;
  /** Original UTF-8 range that owns generated text in this edit. */
  original: { start: number; end: number };
}

/** Stable encoding of a declaration's original-coordinate identity. */
export function identityKey(identity: NativeAssetRecord["identity"]): string {
  const serialized = JSON.stringify({
    kind: identity.kind,
    moduleId: identity.moduleId,
    start: identity.start,
    end: identity.end,
    bindingName: identity.bindingName,
  });
  return `stillwind-identity/v1:${Buffer.from(serialized).toString("base64url")}`;
}

/** Converts a validated original UTF-8 byte boundary to a UTF-16 offset. */
export function byteToUtf16(text: string, offset: number): number {
  const bytes = Buffer.from(text);
  if (
    offset < 0 ||
    offset > bytes.length ||
    (offset < bytes.length && (bytes[offset] & 0xc0) === 0x80)
  ) {
    throw new Error("Invalid original UTF-8 boundary");
  }
  return bytes.subarray(0, offset).toString("utf8").length;
}

/** Checker file identity; the original module id remains unchanged outside the checker. */
export function checkerId(source: PreparedSource): string {
  return source.kind === "direct"
    ? source.checkerFileName
    : `${source.checkerFileName}.${source.projection.syntax.language}`;
}

/** Owns shadow edits and translates checker UTF-16 positions back to original bytes. */
export class CheckerSource {
  readonly source: TypeFactSource;
  readonly buffer: CheckerBuffer;
  readonly text: string;
  readonly id: string;
  readonly identities = new Map<string, NativeAssetRecord["identity"]>();
  /** Independent module that supplies helper declarations in the virtual project. */
  readonly helperModule?: { id: string; text: string };
  /** Shadow UTF-16 AST ranges keyed to original UTF-8 probe ranges. */
  readonly probes = new Map<string, { start: number; end: number; recognitionOnly?: boolean }>();
  /** Producer call ranges after identity insertion, keyed to their asset families. */
  readonly producers = new Map<string, NativeAssetRecord["identity"]["kind"]>();

  readonly #edits: ShadowEdit[] = [];
  readonly #original: string;
  readonly #points: Map<number, number | null>;

  constructor(
    source: TypeFactSource,
    buffer: CheckerBuffer,
    index: number,
    assets: readonly NativeAssetRecord[],
    ids: ReadonlyMap<string, string>,
  ) {
    this.source = source;
    this.buffer = buffer;
    this.id = buffer.region
      ? `${source.checkerFileName}.stillwind-client-${index}.ts`
      : checkerId(source);
    this.#original = source.document.code;
    this.#points = projectionPoints(source);
    const producers = new Map<number, string>();
    const producerRanges: Array<{
      start: number;
      end: number;
      kind: NativeAssetRecord["identity"]["kind"];
    }> = [];
    for (const asset of assets) {
      const targets = this.targets(asset.callStart, asset.callEnd, "producer");
      const key = identityKey(asset.identity);
      this.identities.set(key, asset.identity);
      for (const [start, end] of targets) {
        const previous = producers.get(end);
        if (previous) {
          if (previous !== key) {
            throw new Error(`Conflicting producer identities in ${source.moduleId}`);
          }
          continue;
        }
        producers.set(end, key);
        producerRanges.push({ start, end, kind: asset.identity.kind });
        const call = buffer.source.slice(start, end);
        if (!call.endsWith(")")) {
          throw new Error(`Cannot locate producer in ${source.moduleId}`);
        }
        const separator = [...call.matchAll(/[,(]/g)].some(
          (match) => skipTrivia(call, match.index + 1) === call.length - 1,
        )
          ? ""
          : ",";
        const identity = `{readonly __stillwindDeclarationIdentity:${JSON.stringify(key)}}`;
        const syntax = source.kind === "direct" ? source.syntax : source.projection.syntax;
        const javascript = syntax.language === "js" || syntax.language === "jsx";
        const argument = javascript
          ? `/** @type {${identity}} */ (/** @type {unknown} */ (undefined))`
          : `undefined as unknown as ${identity}`;
        this.#edits.push({
          at: end - 1,
          end: end - 1,
          original: { start: asset.callStart, end: asset.callEnd },
          text: `${separator}${argument}`,
        });
      }
    }
    for (const resolution of source.resolutions ?? []) {
      for (const [at, end] of this.targets(resolution.start, resolution.end, "moduleSpecifier")) {
        this.#edits.push({
          at,
          end,
          original: { start: resolution.start, end: resolution.end },
          text: JSON.stringify(
            path.resolve(ids.get(resolution.id) ?? resolution.id).replaceAll("\\", "/"),
          ),
        });
      }
    }
    const bindings = buffer.region ? [] : (source.checkerBindings ?? []);
    if (bindings.length > 0) {
      const names = new Set(
        buffer.source
          .replaceAll(/\\u(?:\{([\da-fA-F]{1,6})\}|([\da-fA-F]{4}))/g, (escape, braced, fixed) => {
            const value = Number.parseInt(braced ?? fixed, 16);
            return value <= 0x10ffff ? String.fromCodePoint(value) : escape;
          })
          .match(/[$_\p{ID_Start}][$\u200c\u200d\p{ID_Continue}]*/gu),
      );
      const aliases = new Map<string, string>();
      let next = 0;
      for (const binding of bindings) {
        let alias = aliases.get(binding.name);
        if (!alias) {
          do {
            alias = `__stillwindCheckerHelper${next++}`;
          } while (names.has(alias));
          aliases.set(binding.name, alias);
        }
        this.#edits.push({
          at: binding.projectedStart,
          end: binding.projectedEnd,
          text: alias,
          original: { start: 0, end: 0 },
        });
      }
      const id = `${this.id}.stillwind-helpers.ts`;
      this.helperModule = {
        id,
        text: [...aliases.keys()]
          .map((name) => `export declare const ${name}: typeof globalThis.${name};`)
          .join("\n"),
      };
      // Triple-slash directives must precede module statements.
      this.#edits.push({
        at: buffer.source.length,
        end: buffer.source.length,
        text: `\nimport {${[...aliases].map(([name, alias]) => `${name} as ${alias}`).join(",")}} from ${JSON.stringify(path.resolve(id).replaceAll("\\", "/"))};\n`,
        original: { start: 0, end: 0 },
      });
    }
    this.#edits.sort((a, b) => a.at - b.at || a.end - b.end);
    for (let i = 1; i < this.#edits.length; i++) {
      if (
        this.#edits[i].at < this.#edits[i - 1].end ||
        this.#edits[i].at === this.#edits[i - 1].at
      ) {
        throw new Error(`Overlapping checker edits in ${source.moduleId}`);
      }
    }
    for (const probe of source.probes) {
      for (const [start, end] of this.targets(probe.start, probe.end, "expression")) {
        this.probes.set(`${this.toShadow(start)}:${this.toShadow(end, true)}`, probe);
      }
    }
    for (const producer of producerRanges) {
      this.producers.set(
        `${this.toShadow(producer.start)}:${this.toShadow(producer.end, true)}`,
        producer.kind,
      );
    }
    let text = buffer.source;
    for (const edit of [...this.#edits].reverse()) {
      text = text.slice(0, edit.at) + edit.text + text.slice(edit.end);
    }
    this.text = text;
  }

  /** Finds input-buffer UTF-16 ranges for an original UTF-8 range. */
  targets(start: number, end: number, role: string): Array<[number, number]> {
    const a = byteToUtf16(this.#original, start),
      b = byteToUtf16(this.#original, end);
    if (this.buffer.region) {
      return a >= this.buffer.region.start && b <= this.buffer.region.end
        ? [[a - this.buffer.region.start, b - this.buffer.region.start]]
        : [];
    }
    if (
      this.source.checkerBuffers?.some(
        (buffer) => buffer.region && a >= buffer.region.start && b <= buffer.region.end,
      )
    ) {
      return [];
    }
    if (this.source.kind === "direct") {
      return [[a, b]];
    }
    return [
      ...new Map(
        (this.source.checkerSpans ?? [])
          .filter((span) => span.start === start && span.end === end && span.role === role)
          .map((span) => [
            `${span.projectedStart}:${span.projectedEnd}`,
            [span.projectedStart, span.projectedEnd] as [number, number],
          ]),
      ).values(),
    ];
  }

  /** Maps an input checker buffer UTF-16 boundary through shadow edits. */
  toShadow(position: number, end = false): number {
    let delta = 0;
    for (const edit of this.#edits) {
      if (edit.at > position || (edit.at === position && (end || edit.end > edit.at))) {
        break;
      }
      delta += edit.text.length - (edit.end - edit.at);
    }
    return position + delta;
  }

  /** Maps a checker UTF-16 point to an original UTF-8 byte offset, if authored. */
  originalOffset(position: number, end = false): number | null {
    let delta = 0;
    for (const edit of this.#edits) {
      const start = edit.at + delta;
      if (position < start) {
        break;
      }
      if (position < start + edit.text.length) {
        return end ? edit.original.end : edit.original.start;
      }
      delta += edit.text.length - (edit.end - edit.at);
    }
    const generated = position - delta;
    const original = this.buffer.region
      ? generated + this.buffer.region.start
      : this.source.kind === "direct"
        ? generated
        : this.#points.get(generated);
    return original === undefined || original === null
      ? null
      : Buffer.byteLength(this.#original.slice(0, original));
  }
}

/** Exact mapped UTF-16 points for diagnostics; conflicting mappings stay unmapped. */
function projectionPoints(source: PreparedSource): Map<number, number | null> {
  const points = new Map<number, number | null>();
  const add = (generated: number, original: number) => {
    if (points.has(generated) && points.get(generated) !== original) {
      points.set(generated, null);
    } else {
      points.set(generated, original);
    }
  };
  if (source.kind === "direct") {
    return points;
  }
  const map = source.projection.map;
  if (map.kind === "volar") {
    for (const entry of map.mappings) {
      for (let i = 0; i < entry.sourceOffsets.length; i++) {
        const length = entry.generatedLengths?.[i] ?? entry.lengths[i];
        add(entry.generatedOffsets[i], entry.sourceOffsets[i]);
        if (length === entry.lengths[i]) {
          for (let j = 1; j < length; j++) {
            add(entry.generatedOffsets[i] + j, entry.sourceOffsets[i] + j);
          }
        }
      }
    }
  } else {
    const lines = (text: string) => [
      0,
      ...[...text.matchAll(/\n/g)].map((match) => match.index + 1),
    ];
    const generated = lines(source.projection.code),
      original = lines(source.document.code);
    const trace = new TraceMap(map.json);
    const selected = trace.sources.indexOf(map.source);
    for (const [line, segments] of decodedMappings(trace).entries()) {
      for (const segment of segments) {
        if (segment.length >= 4 && segment[1] === selected) {
          add(generated[line] + segment[0], original[segment[2]!] + segment[3]!);
        }
      }
    }
  }
  return points;
}
