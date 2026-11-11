import { minify } from "rolldown/utils";

import { findScripts } from "./scripts.ts";

/**
 * Reports an inline script that cannot be minified or whose minified code cannot stay inside a
 * script element.
 */
export class InlineScriptError extends Error {
  override name = "InlineScriptError";
}

/**
 * Minifies the inline JavaScript of every script element in an HTML document.
 * `filename` names the document in errors. External scripts and data blocks, such as JSON-LD, stay
 * unchanged. A classic script keeps its top-level names because other classic scripts can read
 * them as globals.
 * Returns the document with the minified scripts.
 * Rejects with an {@link InlineScriptError} when a script does not parse or when the minified code
 * contains `</script` or `<!--`.
 */
export async function minifyInlineScripts(html: string, filename: string): Promise<string> {
  const scripts = findScripts(html);
  const replacements = await Promise.all(
    scripts.map(async ({ start, end, attributes, code, inline }) => {
      if (!inline || !code.trim()) {
        return html.slice(start, end);
      }
      // The minifier selects the parser from the file extension, so an HTML filename would fail.
      const result = await minify(inline === "module" ? "inline.mjs" : "inline.js", code, {
        module: inline === "module",
      });
      if (result.errors.length > 0) {
        throw new InlineScriptError(
          `Cannot minify an inline script in ${filename}: ${result.errors
            .map((error) => error.message)
            .join("; ")}`,
        );
      }
      // The HTML parser ends the element at `</script`, and `<!--` can change where it ends.
      if (/<\/script|<!--/i.test(result.code)) {
        throw new InlineScriptError(
          `The minified inline script in ${filename} contains "</script" or "<!--".`,
        );
      }
      return `<script${attributes}>${result.code.trimEnd()}</script>`;
    }),
  );

  let output = "";
  let offset = 0;
  scripts.forEach(({ start, end }, index) => {
    output += html.slice(offset, start) + replacements[index];
    offset = end;
  });
  return output + html.slice(offset);
}
