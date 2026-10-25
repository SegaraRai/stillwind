// Ported from Tailwind CSS 4.3.3 (MIT), utils/infer-data-type.ts and utils/is-color.ts. Keep
// the checks aligned with upstream: they decide which `matchUtilities` registration receives an
// arbitrary value without a type hint, which is a different question from whether the value is
// valid CSS for a property.

import { splitTopLevel } from "./css.ts";

const NAMED_COLORS = new Set([
  // CSS Level 1 colors
  "black",
  "silver",
  "gray",
  "white",
  "maroon",
  "red",
  "purple",
  "fuchsia",
  "green",
  "lime",
  "olive",
  "yellow",
  "navy",
  "blue",
  "teal",
  "aqua",

  // CSS Level 2/3 colors
  "aliceblue",
  "antiquewhite",
  "aqua",
  "aquamarine",
  "azure",
  "beige",
  "bisque",
  "black",
  "blanchedalmond",
  "blue",
  "blueviolet",
  "brown",
  "burlywood",
  "cadetblue",
  "chartreuse",
  "chocolate",
  "coral",
  "cornflowerblue",
  "cornsilk",
  "crimson",
  "cyan",
  "darkblue",
  "darkcyan",
  "darkgoldenrod",
  "darkgray",
  "darkgreen",
  "darkgrey",
  "darkkhaki",
  "darkmagenta",
  "darkolivegreen",
  "darkorange",
  "darkorchid",
  "darkred",
  "darksalmon",
  "darkseagreen",
  "darkslateblue",
  "darkslategray",
  "darkslategrey",
  "darkturquoise",
  "darkviolet",
  "deeppink",
  "deepskyblue",
  "dimgray",
  "dimgrey",
  "dodgerblue",
  "firebrick",
  "floralwhite",
  "forestgreen",
  "fuchsia",
  "gainsboro",
  "ghostwhite",
  "gold",
  "goldenrod",
  "gray",
  "green",
  "greenyellow",
  "grey",
  "honeydew",
  "hotpink",
  "indianred",
  "indigo",
  "ivory",
  "khaki",
  "lavender",
  "lavenderblush",
  "lawngreen",
  "lemonchiffon",
  "lightblue",
  "lightcoral",
  "lightcyan",
  "lightgoldenrodyellow",
  "lightgray",
  "lightgreen",
  "lightgrey",
  "lightpink",
  "lightsalmon",
  "lightseagreen",
  "lightskyblue",
  "lightslategray",
  "lightslategrey",
  "lightsteelblue",
  "lightyellow",
  "lime",
  "limegreen",
  "linen",
  "magenta",
  "maroon",
  "mediumaquamarine",
  "mediumblue",
  "mediumorchid",
  "mediumpurple",
  "mediumseagreen",
  "mediumslateblue",
  "mediumspringgreen",
  "mediumturquoise",
  "mediumvioletred",
  "midnightblue",
  "mintcream",
  "mistyrose",
  "moccasin",
  "navajowhite",
  "navy",
  "oldlace",
  "olive",
  "olivedrab",
  "orange",
  "orangered",
  "orchid",
  "palegoldenrod",
  "palegreen",
  "paleturquoise",
  "palevioletred",
  "papayawhip",
  "peachpuff",
  "peru",
  "pink",
  "plum",
  "powderblue",
  "purple",
  "rebeccapurple",
  "red",
  "rosybrown",
  "royalblue",
  "saddlebrown",
  "salmon",
  "sandybrown",
  "seagreen",
  "seashell",
  "sienna",
  "silver",
  "skyblue",
  "slateblue",
  "slategray",
  "slategrey",
  "snow",
  "springgreen",
  "steelblue",
  "tan",
  "teal",
  "thistle",
  "tomato",
  "turquoise",
  "violet",
  "wheat",
  "white",
  "whitesmoke",
  "yellow",
  "yellowgreen",

  // Keywords
  "transparent",
  "currentcolor",

  // System colors
  "canvas",
  "canvastext",
  "linktext",
  "visitedtext",
  "activetext",
  "buttonface",
  "buttontext",
  "buttonborder",
  "field",
  "fieldtext",
  "highlight",
  "highlighttext",
  "selecteditem",
  "selecteditemtext",
  "mark",
  "marktext",
  "graytext",
  "accentcolor",
  "accentcolortext",
]);

const IS_COLOR_FN = /^(rgba?|hsla?|hwb|color|(ok)?(lab|lch)|light-dark|color-mix|--alpha)\(/i;

function isColor(value: string): boolean {
  return value.startsWith("#") || IS_COLOR_FN.test(value) || NAMED_COLORS.has(value.toLowerCase());
}

const MATH_FUNCTIONS = [
  "calc",
  "min",
  "max",
  "clamp",
  "mod",
  "rem",
  "sin",
  "cos",
  "tan",
  "asin",
  "acos",
  "atan",
  "atan2",
  "pow",
  "sqrt",
  "hypot",
  "log",
  "exp",
  "round",
];

function hasMathFn(input: string): boolean {
  return input.includes("(") && MATH_FUNCTIONS.some((fn) => input.includes(`${fn}(`));
}

export type DataType =
  | "color"
  | "length"
  | "percentage"
  | "ratio"
  | "number"
  | "integer"
  | "url"
  | "position"
  | "bg-size"
  | "line-width"
  | "image"
  | "family-name"
  | "generic-name"
  | "absolute-size"
  | "relative-size"
  | "angle"
  | "vector";

const checks: Record<DataType, (value: string) => boolean> = {
  color: isColor,
  length: isLength,
  percentage: isPercentage,
  ratio: isFraction,
  number: isNumber,
  integer: isPositiveInteger,
  url: isUrl,
  position: isBackgroundPosition,
  "bg-size": isBackgroundSize,
  "line-width": isLineWidth,
  image: isImage,
  "family-name": isFamilyName,
  "generic-name": isGenericName,
  "absolute-size": isAbsoluteSize,
  "relative-size": isRelativeSize,
  angle: isAngle,
  vector: isVector,
};

/**
 * Determine the type of `value` using syntax rules from CSS specs.
 *
 * @param value - Decoded arbitrary value without a type hint.
 * @param types - Types of the registration, in its order.
 * @returns The first type that the value has, or null. A `var()` value has no type.
 */
export function inferDataType(value: string, types: DataType[]): DataType | null {
  if (value.startsWith("var(")) {
    return null;
  }

  for (const type of types) {
    if (checks[type]?.(value)) {
      return type;
    }
  }

  return null;
}

/* -------------------------------------------------------------------------- */

const IS_URL = /^url\(.*\)$/;

function isUrl(value: string): boolean {
  return IS_URL.test(value);
}

/* -------------------------------------------------------------------------- */

function isLineWidth(value: string): boolean {
  return splitTopLevel(value, " ").every(
    (value) =>
      isLength(value) ||
      isNumber(value) ||
      value === "thin" ||
      value === "medium" ||
      value === "thick",
  );
}

/* -------------------------------------------------------------------------- */

const IS_IMAGE_FN = /^(?:element|image|cross-fade|image-set)\(/;
const IS_GRADIENT_FN = /^(repeating-)?(conic|linear|radial)-gradient\(/;

function isImage(value: string) {
  let count = 0;

  for (const part of splitTopLevel(value, ",")) {
    if (part.startsWith("var(")) {
      continue;
    }

    if (isUrl(part)) {
      count += 1;
      continue;
    }

    if (IS_GRADIENT_FN.test(part)) {
      count += 1;
      continue;
    }

    if (IS_IMAGE_FN.test(part)) {
      count += 1;
      continue;
    }

    return false;
  }

  return count > 0;
}

/* -------------------------------------------------------------------------- */

function isGenericName(value: string): boolean {
  return (
    value === "serif" ||
    value === "sans-serif" ||
    value === "monospace" ||
    value === "cursive" ||
    value === "fantasy" ||
    value === "system-ui" ||
    value === "ui-serif" ||
    value === "ui-sans-serif" ||
    value === "ui-monospace" ||
    value === "ui-rounded" ||
    value === "math" ||
    value === "emoji" ||
    value === "fangsong"
  );
}

function isFamilyName(value: string): boolean {
  let count = 0;

  for (const part of splitTopLevel(value, ",")) {
    // If it starts with a digit, then it's not a family name
    const char = part.charCodeAt(0);
    if (char >= 48 && char <= 57) {
      return false;
    }

    if (part.startsWith("var(")) {
      continue;
    }

    count += 1;
  }

  return count > 0;
}

function isAbsoluteSize(value: string): boolean {
  return (
    value === "xx-small" ||
    value === "x-small" ||
    value === "small" ||
    value === "medium" ||
    value === "large" ||
    value === "x-large" ||
    value === "xx-large" ||
    value === "xxx-large"
  );
}

function isRelativeSize(value: string): boolean {
  return value === "larger" || value === "smaller";
}

/* -------------------------------------------------------------------------- */

const HAS_NUMBER = /[+-]?\d*\.?\d+(?:[eE][+-]?\d+)?/;

/* -------------------------------------------------------------------------- */

const IS_NUMBER = new RegExp(`^${HAS_NUMBER.source}$`);

function isNumber(value: string): boolean {
  return IS_NUMBER.test(value) || hasMathFn(value);
}

/* -------------------------------------------------------------------------- */

const IS_PERCENTAGE = new RegExp(`^${HAS_NUMBER.source}%$`);

function isPercentage(value: string): boolean {
  return IS_PERCENTAGE.test(value) || hasMathFn(value);
}

/* -------------------------------------------------------------------------- */

const IS_FRACTION = new RegExp(`^${HAS_NUMBER.source}\\s*/\\s*${HAS_NUMBER.source}$`);

function isFraction(value: string): boolean {
  return IS_FRACTION.test(value) || hasMathFn(value);
}

/* -------------------------------------------------------------------------- */

/**
 * Please refer to MDN when updating this list:
 * @see https://developer.mozilla.org/en-US/docs/Learn/CSS/Building_blocks/Values_and_units
 * @see https://developer.mozilla.org/en-US/docs/Web/CSS/CSS_Container_Queries#container_query_length_units
 */
const LENGTH_UNITS = [
  "cm",
  "mm",
  "Q",
  "in",
  "pc",
  "pt",
  "px",
  "em",
  "ex",
  "ch",
  "rem",
  "lh",
  "rlh",
  "vw",
  "vh",
  "vmin",
  "vmax",
  "vb",
  "vi",
  "svw",
  "svh",
  "lvw",
  "lvh",
  "dvw",
  "dvh",
  "cqw",
  "cqh",
  "cqi",
  "cqb",
  "cqmin",
  "cqmax",
];

const IS_LENGTH = new RegExp(`^${HAS_NUMBER.source}(${LENGTH_UNITS.join("|")})$`);
const IS_LENGTH_FN = /^(--spacing)\(/i;

function isLength(value: string): boolean {
  return IS_LENGTH.test(value) || IS_LENGTH_FN.test(value) || hasMathFn(value);
}

/* -------------------------------------------------------------------------- */

function isBackgroundPosition(value: string): boolean {
  let count = 0;

  for (const part of splitTopLevel(value, " ")) {
    if (
      part === "center" ||
      part === "top" ||
      part === "right" ||
      part === "bottom" ||
      part === "left"
    ) {
      count += 1;
      continue;
    }

    if (part.startsWith("var(")) {
      continue;
    }

    if (isLength(part) || isPercentage(part)) {
      count += 1;
      continue;
    }

    return false;
  }

  return count > 0;
}

/* -------------------------------------------------------------------------- */

/**
 * Recognizes background-size entries for Tailwind type inference.
 *
 * @param value - CSS value text with comma-separated entries.
 * @returns Whether at least one entry is cover, contain, or one or two size values.
 */
function isBackgroundSize(value: string) {
  let count = 0;

  for (const size of splitTopLevel(value, ",")) {
    if (size === "cover" || size === "contain") {
      count += 1;
      continue;
    }

    const values = splitTopLevel(size, " ");

    // Sizes must have exactly one or two values
    if (values.length !== 1 && values.length !== 2) {
      return false;
    }

    if (values.every((value) => value === "auto" || isLength(value) || isPercentage(value))) {
      count += 1;
    }
  }

  return count > 0;
}

/* -------------------------------------------------------------------------- */

const ANGLE_UNITS = ["deg", "rad", "grad", "turn"];

const IS_ANGLE = new RegExp(`^${HAS_NUMBER.source}(${ANGLE_UNITS.join("|")})$`);

/**
 * Recognizes a number with an angle unit.
 *
 * @param value - CSS text without surrounding whitespace.
 * @returns Whether the number ends with deg, rad, grad, or turn.
 */
function isAngle(value: string) {
  return IS_ANGLE.test(value);
}

/* -------------------------------------------------------------------------- */

const IS_VECTOR = new RegExp(`^${HAS_NUMBER.source} +${HAS_NUMBER.source} +${HAS_NUMBER.source}$`);

/**
 * Recognizes the three numbers of a rotate vector.
 *
 * @param value - CSS text without surrounding whitespace.
 * @returns Whether exactly three numbers are separated by spaces.
 */
function isVector(value: string) {
  return IS_VECTOR.test(value);
}

/**
 * Recognizes integer text used by Tailwind type inference.
 *
 * @param value - Number text without surrounding whitespace.
 * @returns True for zero or a positive integer whose JavaScript number spelling matches value.
 */
function isPositiveInteger(value: string): boolean {
  const number = Number(value);
  return Number.isInteger(number) && number >= 0 && String(number) === String(value);
}
