import type { ProjectInput, ScriptSyntax, SourceDocument } from "@stillwind/projection";

const formatsByExtension: Partial<Record<string, SourceDocument["format"]>> = {
  ".jsx": "jsx",
  ".tsx": "jsx",
  ".astro": "astro",
  ".svelte": "svelte",
  ".vue": "vue",
};

const languagesByExtension: Partial<Record<string, ScriptSyntax["language"]>> = {
  ".ts": "ts",
  ".cts": "ts",
  ".mts": "ts",
  ".tsx": "tsx",
  ".jsx": "jsx",
};

/** Source identities and parser metadata supplied by a host before preparation or compilation. */
export type ModuleDescriptor = Pick<
  ProjectInput,
  "moduleId" | "checkerFileName" | "projectionFileName" | "syntax" | "framework"
> & {
  /** Original document identity and format. The compiler obtains its text through the host. */
  document: Omit<SourceDocument, "code">;
};

/**
 * Infers the source format and parser language from a module path.
 *
 * @param id - Source identity whose query or fragment is removed before case-insensitive extension
 * matching.
 * @returns Parser settings with unambiguous module kind and the inferred source format. TypeScript
 * and JSX extensions choose their parser languages. Declaration files use `dts`, and SFC
 * extensions select projected formats. Unrecognized extensions default to JavaScript script input.
 */
export function fileSourceKind(id: string): {
  syntax: ScriptSyntax;
  format: SourceDocument["format"];
} {
  const name = cleanModuleId(id).toLowerCase();
  const extension = /\.[^.]+$/.exec(name)?.[0];
  const format = formatsByExtension[extension ?? ""] ?? "script";
  const language = /\.d\.[cm]?ts$/.test(name)
    ? "dts"
    : (languagesByExtension[extension ?? ""] ?? "js");
  return {
    syntax: {
      language,
      moduleKind: "unambiguous",
    },
    format,
  };
}

/**
 * Removes a filesystem identity's query or fragment before a disk operation.
 *
 * @param id - Resolver identity, including any query or fragment suffix.
 * @returns The identity up to its first `?` or `#`. IDs beginning with NUL or `virtual:` are
 * returned unchanged.
 */
export function cleanModuleId(id: string): string {
  if (id.startsWith("\0") || id.startsWith("virtual:")) {
    return id;
  }
  return id.split(/[?#]/, 1)[0];
}

/**
 * Converts an original UTF-8 byte boundary to a UTF-16 offset for slicing JavaScript text.
 *
 * @param text - Original document text.
 * @param offset - Integer byte offset from zero through the UTF-8 byte length, at a character
 * boundary.
 * @returns Length of the text before that boundary in UTF-16 code units.
 * @throws If the offset is negative, exceeds the byte length, or points into a UTF-8 continuation
 * byte.
 */
export function byteToUtf16(text: string, offset: number): number {
  const bytes = new TextEncoder().encode(text);
  if (
    offset < 0 ||
    offset > bytes.length ||
    (offset < bytes.length && (bytes[offset] & 0xc0) === 0x80)
  ) {
    throw new Error("Invalid original UTF-8 boundary");
  }
  return new TextDecoder().decode(bytes.subarray(0, offset)).length;
}
