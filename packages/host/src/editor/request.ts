import { fingerprint } from "@stillwind/native";

/**
 * A replacement in the original document, using half-open UTF-16 offsets.
 */
export interface EditorEdit {
  /** Inclusive UTF-16 offset in the original document. */
  start: number;
  /** Exclusive UTF-16 offset in the original document. */
  end: number;
  /** Source spelling inserted at the range, including escapes required by its source context. */
  newText: string;
}

/**
 * Completion text replaces the entire candidate at the supplied original range.
 */
export interface EditorCompletion extends EditorEdit {
  /** Directive names use the keyword category. Other inputs use candidate categories. */
  keyword?: boolean;
  /** True for a variant, including CSS variant labels without a trailing colon. */
  variant?: boolean;
  /** True for a label that adds a marker class or starts the modifier of a modifiable marker. */
  marker?: boolean;
  /** Static description for a CSS directive or plugin path. CSS summaries load on resolve. */
  detail?: string;
  /** Replacement encoded as an LSP snippet when the requesting client supports snippets. */
  snippet?: string;
  /** Requests another completion after the client inserts this item. */
  retrigger?: boolean;
  /** Completion category for host-resolved plugin names, files, and directories. */
  category?: "module" | "file" | "directory";
  /** Lexical priority key supplied by the semantic catalog. Omission sorts by label. */
  sortText?: string;
  /** Visible completion text; surrounding group variants are omitted. */
  label: string;
  /** Full candidate used for lazy CSS resolution, including surrounding group variants. */
  candidate: string;
  /** Original UTF-16 input identity used for completion resolution. */
  input: number;
  /** Fingerprint of the captured query inputs. Resolve requires this exact value. */
  snapshot: string;
}

/**
 * A concrete sRGB color at an original candidate range.
 */
export interface EditorColor {
  /** Inclusive UTF-16 offset of the authored candidate. */
  start: number;
  /** Exclusive UTF-16 offset of the authored candidate. */
  end: number;
  /** Red component in the inclusive range 0..1. */
  red: number;
  /** Green component in the inclusive range 0..1. */
  green: number;
  /** Blue component in the inclusive range 0..1. */
  blue: number;
  /** Effective opacity after utility modifiers, in the inclusive range 0..1. */
  alpha: number;
}

/**
 * Requests address original document UTF-16 offsets, including incomplete candidate text.
 */
export type EditorRequest =
  | {
      /** Requests the original definition of a CSS variable or theme reference. */
      kind: "definition";
      /** Original UTF-16 cursor offset within the reference. */
      offset: number;
    }
  | {
      /** Requests plugin module links in the original stylesheet. */
      kind: "links";
    }
  | {
      /** Requests CSS or documentation for a candidate, expression, or CSS directive. */
      kind: "hover";
      /** Original UTF-16 cursor offset at the input to describe. */
      offset: number;
    }
  | {
      /** Requests CSS for every style call, declaration, and use in the source. */
      kind: "previews";
    }
  | {
      /** Requests matching candidates and variant prefixes for an insertion point. */
      kind: "completion";
      /** Includes CSS registry directives. Defaults to false and does not affect compilation. */
      registryDirectives?: boolean;
      /** Includes placeholders for clients that support completion snippets. */
      snippets?: boolean;
      /** Original UTF-16 insertion point; text before this point determines the prefix. */
      offset: number;
    }
  | {
      /** Requests CSS or documentation for a completion from the same captured snapshot. */
      kind: "resolve";
      /** CSS discovery setting used for the original completion. A change makes it stale. */
      registryDirectives?: boolean;
      /** Snippet setting captured when completion was requested. */
      snippets?: boolean;
      /** Full candidate or CSS label returned by completion. */
      candidate: string;
      /** Original UTF-16 boundary returned as `EditorCompletion.input`. */
      input: number;
      /** Captured fingerprint returned by completion; a mismatch omits hover output. */
      snapshot: string;
    }
  | {
      /** Requests concrete colors for recognized utility candidates in the source. */
      kind: "colors";
    }
  | {
      /** Requests each report's first fix action for this source revision, with enabled sorting. */
      kind: "fixes";
    }
  | {
      /** Requests configured sorting independently of lint severity. */
      kind: "sort";
      /** Half-open original UTF-16 ranges that sorting must leave untouched. */
      protectedRanges?: readonly {
        /** Inclusive original UTF-16 offset of the protected text. */
        start: number;
        /** Exclusive original UTF-16 offset of the protected text. */
        end: number;
      }[];
    }
  | {
      /** Requests a replacement candidate for a range returned by the colors query. */
      kind: "colorPresentation";
      /** Inclusive original UTF-16 offset returned by the colors query. */
      start: number;
      /** Exclusive original UTF-16 offset returned by the colors query. */
      end: number;
      /** Desired sRGB components and effective opacity, each in 0..1. */
      color: Pick<EditorColor, "red" | "green" | "blue" | "alpha">;
    };

/**
 * Editor data belongs to the exact source fingerprint in revision. Absent fields mean no answer.
 */
export interface EditorResult {
  /** Original CSS definitions with zero-based line and UTF-16 column coordinates. */
  definitions?: {
    uri: string;
    startLine: number;
    startColumn: number;
    endLine: number;
    endColumn: number;
  }[];
  /** Plugin paths at half-open original UTF-16 ranges. An absent target means unresolved or configured-only. */
  links?: {
    start: number;
    end: number;
    target?: string;
  }[];
  /** Fingerprint of the original source text used to compute every range and edit. */
  revision: string;
  /** Formatted CSS at a half-open original UTF-16 range. Generated element subjects use `&`.
   * Lazy completion resolution uses the sentinel range `[0, 0)`.
   */
  hover?: {
    /** Inclusive original UTF-16 boundary, or zero for completion resolution. */
    start: number;
    /** Exclusive original UTF-16 boundary, or zero for completion resolution. */
    end: number;
    /** Generated CSS or an active theme declaration. Empty for documentation-only hovers. */
    css: string;
    /** Short CSS summary or plugin description for the selected completion. */
    detail?: string;
    /** Markdown for CSS directives, output slots, theme references, plugin paths, or marker classes. */
    documentation?: string;
    /** Authored runtime key expressions, deduplicated in dependency traversal order.
     * Absent when the preview has no runtime key expression to explain.
     */
    runtimeNames?: string[];
  };
  /** Formatted CSS at half-open original UTF-16 ranges, in ascending start order. Literal inputs
   * and occurrences inside another listed occurrence are omitted.
   */
  previews?: {
    /** Inclusive original UTF-16 boundary. */
    start: number;
    /** Exclusive original UTF-16 boundary. */
    end: number;
    /** Generated CSS with the required styles and global definitions. It can be empty. */
    css: string;
    /** False when the occurrence or a style that it requires failed to generate CSS. */
    valid: boolean;
    /** Authored runtime key expressions. Absent when the preview has none to explain. */
    runtimeNames?: string[];
  }[];
  /** Prefix-matching candidates with their full replacement ranges. CSS loads on resolve. */
  completions?: EditorCompletion[];
  /** Concrete colors resolved against the active design system. */
  colors?: EditorColor[];
  /** Nonoverlapping replacements in ascending original offset order. */
  edits?: EditorEdit[];
}

/**
 * Starts an editor answer tied to the original source text.
 *
 * @param source - Complete original document text.
 * @returns An empty answer with the fingerprint required for its ranges and edits.
 */
export function editorResult(source: string): EditorResult {
  return {
    revision: fingerprint(source),
  };
}
