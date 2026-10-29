import { cssBannerPrefix } from "stillwind/css";

/**
 * Adds notices before a CSS body without changing the body.
 *
 * @param output - CSS body and the notices that it requires.
 * @returns The published stylesheet text, or an empty string when the body is blank.
 */
export function finalizeCss(output: { css: string; banners?: readonly string[] }): string {
  return output.css.trim() ? cssBannerPrefix(output.banners) + output.css : "";
}
