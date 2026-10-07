import { TraceMap, decodedMappings } from "@jridgewell/trace-mapping";
import type { NativeAssetRecord } from "@stillwind/native";
import type { CheckerBuffer, PreparedSource } from "@stillwind/projection";
import * as path from "pathe";
import ts from "typescript-6";

import { byteToUtf16 } from "../compilation/module-source.ts";
import type { TypeFactSource } from "./backend.ts";

/** Checker-only insertion/replacement in input-buffer UTF-16 coordinates. */
interface ShadowEdit {
  /** Inclusive UTF-16 offset of the insertion or replacement in the unedited checker buffer. */
  at: number;
  /** Exclusive UTF-16 end in that buffer. Equal to `at` for an insertion. */
  end: number;
  /** Generated checker text that replaces the selected interval. */
  text: string;
  /** Original UTF-8 range that owns generated text in this edit. */
  original: {
    /** Inclusive UTF-8 byte offset used to map diagnostics inside the generated text. */
    start: number;
    /** Exclusive UTF-8 byte offset used when mapping the end of such a diagnostic. */
    end: number;
  };
}

/**
 * Serializes a declaration identity as a stable checker string-literal key.
 *
 * @param identity - Native declaration identity with original UTF-8 offsets.
 * @returns `stillwind-identity/v1:` followed by base64url JSON of kind, module ID, range, and
 * binding name in a fixed field order.
 */
export function identityKey(identity: NativeAssetRecord["identity"]): string {
  const serialized = JSON.stringify({
    kind: identity.kind,
    moduleId: identity.moduleId,
    start: identity.start,
    end: identity.end,
    bindingName: identity.bindingName,
  });
  return `stillwind-identity/v1:${base64url(serialized)}`;
}

/**
 * Selects the checker filename for a prepared source.
 *
 * @param source - Source with its host checker filename and any projection syntax.
 * @returns The host filename for direct input, or that filename followed by the projection language
 * extension for projected input.
 */
export function checkerId(source: PreparedSource): string {
  return source.kind === "direct"
    ? source.checkerFileName
    : `${source.checkerFileName}.${source.projection.syntax.language}`;
}

/**
 * Checker buffer with declaration identities, resolved import paths, and helper aliases inserted
 * for type queries. Its range maps relate checker AST expressions to original document bytes.
 */
export class CheckerSource {
  /** Prepared source and original-coordinate requests that own this checker buffer. */
  readonly source: TypeFactSource;
  /** Unedited input buffer, optionally restricted to one original script region. */
  readonly buffer: CheckerBuffer;
  /**
   * Final checker text after identity arguments, module paths, and helper bindings have been
   * rewritten.
   */
  readonly text: string;
  /**
   * Checker filename, with a distinct client-script suffix when the buffer has an original region.
   */
  readonly id: string;
  /** Serialized identity keys mapped to the native declarations supplied during construction. */
  readonly identities = new Map<string, NativeAssetRecord["identity"]>();
  /** Independent module that supplies helper declarations in the virtual project. */
  readonly helperModule?: { id: string; text: string };
  /** Shadow UTF-16 AST ranges keyed to original UTF-8 probe ranges. */
  readonly probes = new Map<string, { start: number; end: number; recognitionOnly?: boolean }>();
  /** Producer call ranges after identity insertion, keyed to their asset families. */
  readonly producers = new Map<string, NativeAssetRecord["identity"]["kind"]>();

  /**
   * Ordered, nonoverlapping edits in the input buffer. Their cumulative lengths translate between
   * input and checker offsets.
   */
  readonly #edits: ShadowEdit[] = [];
  /** Original document text used to convert byte ranges and recover diagnostic byte offsets. */
  readonly #original: string;
  /**
   * Projected UTF-16 positions mapped to original UTF-16 positions. Null marks conflicting
   * correspondences. Missing keys have no known mapping.
   */
  readonly #points: Map<number, number | null>;

  /**
   * Constructs checker text and range indexes without modifying the source or input buffer.
   *
   * @param source - Prepared source with validated frontend correspondences, probes, resolutions,
   * and helper references.
   * @param buffer - Direct or projected checker text, optionally scoped to an original script
   * region.
   * @param index - Buffer index used to give independent client scripts distinct filenames.
   * @param assets - Native declaration records with original-byte producer call ranges. Repeated
   * records for the same generated call are inserted once.
   * @param ids - Resolved source identities mapped to their main checker filenames for import
   * rewriting.
   * @throws If a byte boundary is invalid, producer identities conflict, a mapped producer does not
   * end with a closing parenthesis, edits overlap, or projection decoding fails.
   */
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

  /**
   * Finds input-buffer intervals corresponding to one original range and role.
   *
   * @param start - Inclusive original UTF-8 byte boundary.
   * @param end - Exclusive original UTF-8 byte boundary.
   * @param role - Frontend correspondence role, matched exactly for projected input.
   * @returns Deduplicated UTF-16 intervals. Independent scripts receive only contained ranges.
   * Their ranges are excluded from the main buffer. Unmapped projected ranges produce an empty
   * list.
   * @throws If either original byte boundary is invalid.
   */
  targets(start: number, end: number, role: string): Array<[number, number]> {
    const a = byteToUtf16(this.#original, start);
    const b = byteToUtf16(this.#original, end);
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

  /**
   * Translates an input-buffer boundary after checker edits are applied.
   *
   * @param position - UTF-16 boundary in the unedited checker buffer.
   * @param end - True for an interval end, which stays before an insertion at the same position.
   * False includes such an insertion.
   * @returns The UTF-16 boundary in final checker text. Positions should delimit preserved input
   * intervals rather than lie inside replaced text.
   */
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

  /**
   * Maps a checker position back to an original document byte boundary.
   *
   * @param position - UTF-16 position in final checker text.
   * @param end - True selects the owning original range's end for positions inside generated edits.
   * False selects its start.
   * @returns An original UTF-8 byte offset, or null when a projected position has no unique known
   * correspondence.
   */
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
      : new TextEncoder().encode(this.#original.slice(0, original)).length;
  }
}

/**
 * Collects exact projection points for mapping checker diagnostics back to a document.
 *
 * @param source - Prepared source with its original text and any Volar or version 3 projection map.
 * @returns Generated UTF-16 positions mapped to original UTF-16 positions. Conflicting points map
 * to null. Direct sources produce an empty map because their offsets are translated directly.
 * @throws If a version 3 projection map cannot be decoded.
 */
export function projectionPoints(source: PreparedSource): Map<number, number | null> {
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
    const generated = lines(source.projection.code);
    const original = lines(source.document.code);
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

function base64url(text: string): string {
  return btoa(String.fromCharCode(...new TextEncoder().encode(text)))
    .replaceAll("+", "-")
    .replaceAll("/", "_")
    .replace(/=+/, "");
}

function skipTrivia(text: string, offset: number): number {
  const scanner = ts.createScanner(ts.ScriptTarget.Latest, true);
  scanner.setText(text);
  scanner.resetTokenState(offset);
  scanner.scan();
  return scanner.getTokenStart();
}
