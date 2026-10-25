import {
  fingerprint,
  type NativeEditorSite,
  type NativeIdentity,
  type NativeModuleEditor,
  type NativeProjectSession,
} from "@stillwind/native";
import type { PreparedSource } from "@stillwind/projection";

import { byteToUtf16 } from "../compilation/module-source.ts";
import type { ModuleAnalysis } from "../compilation/request.ts";
import { identityKey } from "../semantics/checker-source.ts";

type CapturedEditorSite = Readonly<
  Omit<NativeEditorSite, "identity" | "dependencies" | "runtimeNames">
> & {
  readonly identity?: Readonly<NativeIdentity>;
  readonly dependencies: readonly Readonly<NativeIdentity>[];
  readonly runtimeNames: readonly string[];
};

/** A compiler occurrence mapped to the original document's UTF-16 coordinates. */
export interface PreviewSite {
  /** Compiler output with original UTF-8 ranges and any literal replacement context. */
  readonly native: CapturedEditorSite;
  /** Inclusive UTF-16 offset in the original document, before projection. */
  readonly start: number;
  /** Exclusive UTF-16 offset in the original document, before projection. */
  readonly end: number;
}

/** CSS for a captured occurrence and any runtime expressions that supply view-transition names. */
export interface GeneratedPreview {
  /** CSS for the occurrence and its required styles and global definitions. */
  css: string;
  /** Authored runtime key expressions, deduplicated in dependency traversal order. */
  runtimeNames: string[];
}

/** Captures compiler input plans and assembles their required declaration previews. */
export class GeneratedPreviews {
  /** Fingerprint of original buffers, capture revisions, and resolved graph dependencies. */
  readonly revision: string;

  /** Captures and original UTF-16 sites, keyed by full compiler module identity. */
  readonly #modules = new Map<
    string,
    {
      /** Frozen module plans whose generated names belong to this capture batch. */
      editor: NativeModuleEditor;
      /** Captured occurrences mapped to original UTF-16 offsets, in compiler traversal order. */
      sites: readonly PreviewSite[];
    }
  >();
  /** Declaration and style-call sites keyed by semantic identity, including imported values. */
  readonly #declarations = new Map<string, CapturedEditorSite>();

  /**
   * Captures the graph's known modules together so their display names share one table.
   * Unknown modules are skipped.
   * Dependency previews require their declaration modules to be present in the graph.
   *
   * @param graph - Original buffers and dependencies for the source revisions to capture.
   * @param session - Session whose analyses for modules match the supplied buffers. Captures remain
   * usable after later session updates. Published sites are readonly.
   * @param modules - Module IDs successfully compiled from these buffers. Other retained analyses are skipped.
   * @throws If a captured UTF-8 range is invalid for its original buffer.
   */
  constructor(
    graph: Pick<ModuleAnalysis, "sources" | "dependencies">,
    session: NativeProjectSession,
    modules: readonly string[],
  ) {
    const revisions: string[] = [];
    const captures = new Map(
      session.editorModules([...modules]).map((editor) => [editor.moduleId(), editor]),
    );
    for (const source of graph.sources) {
      const editor = captures.get(source.moduleId);
      revisions.push(source.moduleId, fingerprint(source.document.code), editor?.revision() ?? "");
      if (!editor) {
        continue;
      }

      const sites: readonly PreviewSite[] = editor.sites().map((native) => ({
        native,
        start: byteToUtf16(source.document.code, native.start),
        end: byteToUtf16(source.document.code, native.end),
      }));
      this.#modules.set(source.moduleId, {
        editor,
        sites,
      });
      for (const { native } of sites) {
        if (native.identity) {
          this.#declarations.set(identityKey(native.identity), native);
        }
      }
    }
    this.revision = fingerprint(JSON.stringify([revisions, graph.dependencies]));
  }

  /**
   * Finds the occurrences captured for one compiler module.
   *
   * @param source - Prepared source whose full module identity selects the capture.
   * @returns Occurrences in compiler traversal order, or an empty list for an unknown module.
   * The array and its entries are readonly and shared with this capture.
   */
  sites(source: PreparedSource): readonly PreviewSite[] {
    return this.#modules.get(source.moduleId)?.sites ?? [];
  }

  /**
   * Finds a literal at the half-open original UTF-16 range `[start, end)`.
   * The range may address the literal content or include its one-character delimiters.
   *
   * @param source - Prepared source whose full module identity selects the capture.
   * @param start - Inclusive original UTF-16 offset of the content or opening delimiter.
   * @param end - Exclusive original UTF-16 offset of the content or closing delimiter.
   * @returns A shared occurrence with an input context, or undefined when no range matches exactly.
   */
  input(source: PreparedSource, start: number, end: number): PreviewSite | undefined {
    for (const site of this.sites(source)) {
      if (
        site.native.context &&
        ((site.start === start && site.end === end) ||
          (site.start === start + 1 && site.end === end - 1))
      ) {
        return site;
      }
    }
    return undefined;
  }

  /**
   * Renders a replacement for the whole captured input while retaining its compiler conditions.
   * The result contains only that plan's CSS. Dependency assembly is handled by `preview`.
   *
   * @param source - Prepared source whose full module identity selects the capture.
   * @param start - Inclusive original UTF-8 byte offset of the captured input, taken from
   * `PreviewSite.native.start`.
   * @param candidate - Decoded replacement text without enclosing group variants.
   * @returns CSS for that plan, or undefined for an unknown module or input, failed emission,
   * or output with no CSS. Captured plans and shared generated names remain unchanged.
   */
  previewInput(source: PreparedSource, start: number, candidate: string): string | undefined {
    return this.#modules.get(source.moduleId)?.editor.previewInput(start, candidate) ?? undefined;
  }

  /**
   * Assembles a captured declaration or style call addressed by its semantic identity.
   * Imported aliases reuse that identity.
   *
   * @param identity - Full native declaration identity, including original UTF-8 ranges.
   * @returns The preview assembled by `preview`, or undefined when the identity is absent
   * or its declaration and dependencies cannot supply valid, nonempty CSS.
   */
  previewIdentity(identity: NativeIdentity): GeneratedPreview | undefined {
    const site = this.#declarations.get(identityKey(identity));
    return site ? this.preview(site) : undefined;
  }

  /**
   * Assembles `site` and its dependencies from this capture batch. Composed styles precede
   * the site's CSS, and global definitions follow it. Each dependency identity is visited once.
   * Identical CSS parts and runtime expression text are deduplicated in traversal order.
   *
   * @param site - Occurrence from this instance's capture batch. Its generated names must share
   * the declaration captures' name table.
   * @returns CSS and runtime key expressions, or undefined if any required site is missing or
   * invalid, or if the assembled CSS is empty. The site and declaration captures remain unchanged.
   */
  preview(site: CapturedEditorSite): GeneratedPreview | undefined {
    const parts: string[] = [];
    const runtimeNames = new Set<string>();
    const seen = new Set<string>();
    /**
     * Appends valid output and dependency expressions to the shared accumulators.
     * A false result can leave partial entries, which the outer query discards on failure.
     *
     * @param site - Occurrence from the same capture batch as its dependency declarations.
     * @returns Whether the occurrence and every newly visited dependency are valid and present.
     */
    const collect = (site: CapturedEditorSite): boolean => {
      if (!site.valid) {
        return false;
      }

      const dependencies = site.dependencies.flatMap((identity) => {
        const key = identityKey(identity);
        if (seen.has(key)) {
          return [];
        }

        seen.add(key);
        return [
          {
            identity,
            site: this.#declarations.get(key),
          },
        ];
      });
      for (const dependency of dependencies.filter((item) => item.identity.kind === "style")) {
        if (!dependency.site || !collect(dependency.site)) {
          return false;
        }
      }

      if (site.css) {
        parts.push(site.css);
      }
      for (const name of site.runtimeNames) {
        runtimeNames.add(name);
      }

      for (const dependency of dependencies.filter((item) => item.identity.kind !== "style")) {
        if (!dependency.site || !collect(dependency.site)) {
          return false;
        }
      }
      return true;
    };
    if (!collect(site)) {
      return undefined;
    }

    const css = [...new Set(parts)].join("\n");
    return css
      ? {
          css,
          runtimeNames: [...runtimeNames],
        }
      : undefined;
  }
}
