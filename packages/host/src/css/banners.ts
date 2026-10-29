/**
 * Formats notices as the prefix of a published stylesheet.
 *
 * @param banners - Notices in published order without duplicates, as a native plan or part lists
 * them. Callers that combine lists must order them again with `cssBanners` from `stillwind/css`.
 * @returns The notices in the given order, each followed by a newline.
 */
export function cssBannerPrefix(banners: readonly string[]): string {
  return banners.map((banner) => `${banner}\n`).join("");
}

/**
 * Adds notices before a CSS body without changing the body.
 *
 * @param output - CSS body and the notices that it requires, in the form that
 * {@link cssBannerPrefix} accepts.
 * @returns The published stylesheet text, or an empty string when the body is blank.
 */
export function finalizeCss(output: { css: string; banners?: readonly string[] }): string {
  return output.css.trim() ? cssBannerPrefix(output.banners ?? []) + output.css : "";
}
