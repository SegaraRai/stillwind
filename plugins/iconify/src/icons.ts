import type { IconifyJSON } from "@iconify/types";
import { getIconData } from "@iconify/utils/lib/icon-set/get-icon";
import { iconToSVG } from "@iconify/utils/lib/svg/build";
import { iconToHTML } from "@iconify/utils/lib/svg/html";
import { svgToURL } from "@iconify/utils/lib/svg/url";

/**
 * How an icon utility paints the icon. `mask` paints the element's text color through the icon's
 * shape, and `background` paints the icon's own colors. `auto` selects `mask` for icons whose SVG
 * uses `currentColor`, and `background` for the others.
 */
export type IconMode = "auto" | "mask" | "background";

/**
 * Custom property that sets the height of an icon box. Size utilities set it, and icon
 * utilities fall back to {@link IconStyle["scale"]} when it has no value.
 */
export const ICON_SIZE = "--sw-icon-size";

/** Settings that apply to every icon utility of a plugin instance. */
export interface IconStyle {
  /** Default icon height in `em`. The width keeps the icon's aspect ratio. */
  readonly scale: number;
  /** Paint method of the icons. */
  readonly mode: IconMode;
}

/**
 * Generates the Stillwind CSS of one icon utility.
 *
 * The SVG becomes a `@theme` token, so the compiled CSS contains the SVG once, and only when a
 * module uses the utility.
 *
 * @param utility - Utility name, such as `icon-mdi--close` or `icon-[mdi--close]`.
 * @param token - Custom property name of the SVG token, such as `--icon-svg-mdi--close`. It must be
 * a valid CSS identifier.
 * @param collection - Icon set that contains the icon.
 * @param name - Icon name or alias in the set.
 * @param style - Size and paint method.
 * @returns The `@theme` block and the `@utility` rule, or undefined when the set has no such icon.
 */
export function iconCss(
  utility: string,
  token: string,
  collection: IconifyJSON,
  name: string,
  style: IconStyle,
): string | undefined {
  const data = getIconData(collection, name);
  if (!data) {
    return undefined;
  }

  const svg = iconToSVG(data, {
    height: "auto",
  });
  const [, , width, height] = svg.viewBox;
  const mask =
    style.mode === "mask" || (style.mode === "auto" && svg.body.includes("currentColor"));
  // Each mode resets the other mode's image, so a later icon utility on the same element
  // replaces the earlier icon even when their modes differ.
  const paint = mask
    ? [
        "background-color: currentColor;",
        "background-image: none;",
        `mask-image: var(${token});`,
        "mask-size: 100% 100%;",
        "mask-repeat: no-repeat;",
      ]
    : [
        "background-color: transparent;",
        `background-image: var(${token});`,
        "background-size: 100% 100%;",
        "background-repeat: no-repeat;",
        "mask-image: none;",
      ];
  return [
    `@theme {`,
    `  ${token}: ${svgToURL(iconToHTML(svg.body, svg.attributes))};`,
    `}`,
    `@utility ${utility} (implementation) {`,
    `  display: inline-block;`,
    // `max-content` derives the width from the height and the aspect ratio. Unlike `auto`,
    // it keeps a flex or grid container from stretching the box across its cross axis.
    `  width: max-content;`,
    `  height: var(${ICON_SIZE}, ${em(style.scale)});`,
    `  aspect-ratio: ${decimal(width)} / ${decimal(height)};`,
    ...paint.map((declaration) => `  ${declaration}`),
    `}`,
    ``,
  ].join("\n");
}

/**
 * Formats a length in `em` with at most four fractional digits.
 *
 * @param value - Finite length.
 * @returns CSS length text.
 */
function em(value: number): string {
  return `${decimal(value)}em`;
}

/**
 * Formats a number with at most four fractional digits.
 *
 * @param value - Finite number.
 * @returns CSS number text.
 */
function decimal(value: number): string {
  return `${Math.round(value * 10_000) / 10_000}`;
}
