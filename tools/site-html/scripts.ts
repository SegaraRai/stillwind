/**
 * A script element in a built HTML document.
 */
export interface ScriptElement {
  /** Offset of `<script` in the document. */
  start: number;
  /** Offset after `</script>` in the document. */
  end: number;
  /** Text between `<script` and `>`, including its leading whitespace. */
  attributes: string;
  /** Raw text between the start tag and the end tag. */
  code: string;
  /**
   * How the browser runs the element's own text: `classic` or `module`. `undefined` marks an
   * external script and a data block, such as JSON-LD, whose text the browser does not run.
   */
  inline: "classic" | "module" | undefined;
}

/**
 * Returns the script elements of an HTML document in document order.
 * The document must come from an HTML serializer: the scan does not skip comments, and it ends
 * each element at the first `</script>`.
 */
export function findScripts(html: string): ScriptElement[] {
  return [...html.matchAll(/<script\b([^>]*)>([\s\S]*?)<\/script>/gi)].map((match) => {
    const [element, attributes = "", code = ""] = match;
    return {
      start: match.index,
      end: match.index + element.length,
      attributes,
      code,
      inline: inlineKind(attributes),
    };
  });
}

function inlineKind(attributes: string): ScriptElement["inline"] {
  if (/(?:^|\s)src(?:\s*=|\s|$)/i.test(attributes)) {
    return undefined;
  }
  const type = /(?:^|\s)type\s*=\s*(?:"([^"]*)"|'([^']*)'|([^\s"'>]+))/i.exec(attributes);
  const value = (type?.[1] ?? type?.[2] ?? type?.[3] ?? "").trim().toLowerCase();
  if (value === "module") {
    return "module";
  }
  return value === "" || value === "text/javascript" ? "classic" : undefined;
}
