import type { IconifyInfo } from "@iconify/types";

/**
 * Formats the supplied set information as a preserved CSS notice.
 *
 * @param info - Metadata of the same source that supplied the SVG, or undefined.
 * @returns A banner declaration, or an empty string when no information was provided.
 * @throws If supplied metadata lacks a set name, author, or license title.
 */
export function iconLicense(info: IconifyInfo | undefined): string {
  if (!info) {
    return "";
  }

  if (
    typeof info.name !== "string" ||
    !info.name.trim() ||
    typeof info.author?.name !== "string" ||
    !info.author.name.trim() ||
    typeof info.license?.title !== "string" ||
    !info.license.title.trim()
  ) {
    throw new Error("Iconify metadata requires a set name, author, and license title");
  }

  const { author, license } = info;
  for (const value of [author.url, license.spdx, license.url]) {
    if (value !== undefined && typeof value !== "string") {
      throw new Error("Iconify author and license metadata must contain strings");
    }
  }

  const parts = [
    info.name,
    author.name,
    author.url,
    license.title,
    license.spdx && license.spdx !== license.title ? license.spdx : undefined,
    license.url,
  ].filter((value): value is string => Boolean(value));
  const text = parts.join(" | ").replaceAll("*/", "* /");
  return `  @stillwind-banner { /*! ${text} */ }`;
}
