import type { NativePluginOption } from "@stillwind/native";

/**
 * Serializes a plugin CSS object, as plugins pass it to Tailwind's API, while preserving
 * repeated declarations and nested rules.
 *
 * @param input - CSS-in-JS object or array of them. An array value of a property repeats the
 * declaration, camel-case property names become kebab-case, and null, undefined, and false
 * values are skipped. `@slot` keys and values become `@slot` statements.
 * @returns CSS declarations and rules without surrounding braces, or an empty string for input
 * that is not an object.
 */
export function serializeCss(input: unknown): string {
  if (Array.isArray(input)) {
    return input.map(serializeCss).join("\n");
  }
  if (!input || typeof input !== "object") {
    return "";
  }
  let output = "";
  for (const [key, value] of Object.entries(input)) {
    if (value === null || value === undefined || value === false) {
      continue;
    }
    if (key === "@slot") {
      output += "@slot;";
      continue;
    }
    if (!key.startsWith("--") && value === "@slot") {
      output += `${key}{@slot;}`;
      continue;
    }
    if (statement(key, value)) {
      output += `${key};`;
      continue;
    }
    const values = Array.isArray(value) ? value : [value];
    for (const entry of values) {
      if (entry && typeof entry === "object") {
        output += `${key}{${serializeCss(entry)}}`;
      } else if (entry !== null && entry !== undefined && entry !== false) {
        const property = key.startsWith("--")
          ? key
          : key.replace(/[A-Z]/g, (letter) => `-${letter.toLowerCase()}`).replace(/^ms-/, "-ms-");
        output += `${property}:${String(entry)};`;
      }
    }
  }
  return output;
}

/**
 * Whether an entry is an at-rule without a block, such as `{ "@apply p-4": {} }`, which
 * Tailwind serializes as the statement `@apply p-4;`.
 *
 * @param key - Entry key.
 * @param value - Entry value.
 * @returns Whether the key is an at-rule and the value an empty object.
 */
function statement(key: string, value: unknown): boolean {
  return (
    key.startsWith("@") &&
    typeof value === "object" &&
    value !== null &&
    !Array.isArray(value) &&
    Object.keys(value).length === 0
  );
}

/**
 * Escapes a complete candidate as one CSS identifier, including arbitrary-value punctuation.
 *
 * @param value - Candidate or variant name, such as `-probe-[3px]/mirror`.
 * @returns An identifier whose CSS escapes decode to `value`. A leading digit becomes a
 * six-digit hexadecimal escape.
 */
export function identifier(value: string): string {
  return Array.from(value)
    .map((character, index) => {
      if (/[_a-zA-Z\u0080-\uffff-]/u.test(character) || (index > 0 && /\d/.test(character))) {
        return character;
      }
      if (/\d/.test(character)) {
        return `\\${character.codePointAt(0)!.toString(16).padStart(6, "0")}`;
      }
      return `\\${character}`;
    })
    .join("");
}

/**
 * Applies an alpha value to a color as Tailwind's `withAlpha()` does.
 *
 * Every plain number is multiplied by 100, so `0.5` means 50% and `50` means 5000%, which
 * `color-mix()` rejects. Tailwind applies this to the `/50` of `theme("colors.red.500 / 50")`,
 * while a `/50` color modifier of a utility arrives here already as `50%`.
 *
 * @param value - CSS color.
 * @param alpha - Number, percentage, or other CSS value such as `var(--opacity)`.
 * @returns The color itself for `100%`, and a `color-mix()` toward transparent otherwise.
 */
export function withAlpha(value: string, alpha: string): string {
  const number = Number(alpha);
  const percentage = Number.isNaN(number) ? alpha : `${number * 100}%`;
  return percentage === "100%" ? value : `color-mix(in oklab, ${value} ${percentage}, transparent)`;
}

/** Class names that Tailwind's `addUtilities()` registers. */
const IS_VALID_UTILITY_NAME = /^[a-z@][a-zA-Z0-9/%._-]*$/;

/**
 * Finds the classes that a plugin selector registers, as Tailwind's `addUtilities()` does: every
 * class outside attribute selectors and outside the arguments of `:not()`, `:nth-child()`, and
 * `:nth-last-child()`, whose classes are conditions rather than utilities. `.outer .inner`
 * registers both `outer` and `inner`.
 *
 * @param selector - One complex selector without top-level commas.
 * @returns The decoded class names with the range of each `.name`, in source order.
 */
function utilityClasses(selector: string): { name: string; start: number; end: number }[] {
  const classes: {
    name: string;
    start: number;
    end: number;
  }[] = [];
  const parentheses: boolean[] = [];
  let excluded = 0;
  let attribute = 0;
  let quote = "";
  for (let index = 0; index < selector.length; index++) {
    const char = selector[index];
    if (char === "\\") {
      index++;
    } else if (quote) {
      quote = char === quote ? "" : quote;
    } else if (char === '"' || char === "'") {
      quote = char;
    } else if (char === "[") {
      attribute++;
    } else if (char === "]") {
      attribute--;
    } else if (char === "(") {
      const condition = /:(?:not|nth-child|nth-last-child)$/i.test(selector.slice(0, index));
      parentheses.push(condition);
      excluded += condition ? 1 : 0;
    } else if (char === ")") {
      excluded -= parentheses.pop() ? 1 : 0;
    } else if (char === "." && !excluded && !attribute) {
      const { name, end } = readIdentifier(selector, index + 1);
      if (IS_VALID_UTILITY_NAME.test(name)) {
        classes.push({
          name,
          start: index,
          end,
        });
      }
      index = end - 1;
    }
  }
  return classes;
}

/**
 * Reads a CSS identifier and decodes its escapes.
 *
 * @param text - Text that contains the identifier.
 * @param start - Index of the first character of the identifier.
 * @returns The decoded name, empty when no identifier starts at `start`, and the index after it.
 */
function readIdentifier(text: string, start: number): { name: string; end: number } {
  let end = start;
  let name = "";
  while (end < text.length) {
    const next = text[end];
    if (next === "\\") {
      const hex = /^[0-9a-f]{1,6}\s?/i.exec(text.slice(end + 1));
      name += hex ? String.fromCodePoint(Number.parseInt(hex[0], 16)) : text[end + 1];
      end += 1 + (hex ? hex[0].length : 1);
    } else if (/[\w\u0080-￿-]/u.test(next)) {
      name += next;
      end++;
    } else {
      break;
    }
  }
  return {
    name,
    end,
  };
}

/**
 * Serializes text as a CSS string.
 *
 * @param value - Text, such as generated CSS.
 * @returns A double-quoted CSS string whose escapes decode to `value`.
 */
export function cssString(value: string): string {
  return `"${value
    .replace(/["\\]/g, "\\$&")
    .replace(/[\n\r\f]/g, (character) => `\\${character.codePointAt(0)!.toString(16)} `)}"`;
}

/**
 * Replaces every `&` outside strings and attribute selectors.
 *
 * @param selector - Selector that may contain `&`.
 * @param parent - Replacement for `&`.
 * @returns The selector with the parent substituted, or undefined when it has no `&`.
 */
function substituteNesting(selector: string, parent: string): string | undefined {
  let output = "";
  let nested = false;
  let attribute = 0;
  let quote = "";
  for (let index = 0; index < selector.length; index++) {
    const char = selector[index];
    if (char === "\\") {
      output += selector.slice(index, index + 2);
      index++;
      continue;
    }
    if (quote) {
      quote = char === quote ? "" : quote;
    } else if (char === '"' || char === "'") {
      quote = char;
    } else if (char === "[") {
      attribute++;
    } else if (char === "]") {
      attribute--;
    } else if (char === "&" && !attribute) {
      output += parent;
      nested = true;
      continue;
    }
    output += char;
  }
  return nested ? output : undefined;
}

/**
 * Converts the rules that `addUtilities()` receives into utility bodies. At-rules retain their
 * enclosing conditions.
 *
 * @param input - CSS-in-JS object keyed by selectors and at-rules, or an array of them.
 * @param register - Receives the anchor class of a selector and one body for it, once for each
 * selector of the body.
 * @param global - Receives top-level `@keyframes` and `@property` rules, which apply whether or
 * not a utility is used. The compiler emits such rules nested in a utility body only with that
 * utility.
 * @param wrappers - Enclosing at-rule preludes, outermost first.
 * @throws If a selector has no class outside `:not()` and attribute selectors.
 */
export function utilityRules(
  input: unknown,
  register: (name: string, body: string) => void,
  global: (css: string) => void,
  wrappers: string[] = [],
): void {
  if (Array.isArray(input)) {
    for (const entry of input) {
      utilityRules(entry, register, global, wrappers);
    }
    return;
  }
  if (!input || typeof input !== "object") {
    return;
  }
  /**
   * Wraps CSS in the enclosing at-rules.
   * @param body - CSS inside the innermost at-rule.
   * @returns The wrapped CSS.
   */
  const wrap = (body: string) => wrappers.reduceRight((css, rule) => `${rule}{${css}}`, body);
  for (const [selector, body] of Object.entries(input)) {
    if (selector.startsWith("@keyframes ") || selector.startsWith("@property ")) {
      global(wrap(`${selector}{${serializeCss(body)}}`));
    } else if (selector.startsWith("@")) {
      utilityRules(body, register, global, [...wrappers, selector]);
    } else {
      for (const branch of splitTopLevel(selector, ",").map((part) => part.trim())) {
        const classes = utilityClasses(branch);
        if (!classes.length) {
          throw new Error(`Tailwind plugin utility requires a class selector: ${selector}`);
        }
        // Each class becomes `&` in its own registration and stays literal in the others.
        for (const anchor of classes) {
          const relative = `${branch.slice(0, anchor.start)}&${branch.slice(anchor.end)}`;
          for (const css of utilityBodies(body, relative, wrappers)) {
            register(anchor.name, css);
          }
        }
      }
    }
  }
}

/**
 * Lists the selectors of the nested rules in CSS-in-JS input, each as an empty rule, so that
 * the compiler can collect the classes that they name without the declarations.
 *
 * @param input - CSS-in-JS body, or an array of them.
 * @returns Each selector key as written followed by `{}`, in source order. Selectors inside
 * conditional at-rules are included, and `@keyframes` and `@property` rules are skipped.
 */
export function selectorRules(input: unknown): string[] {
  if (Array.isArray(input)) {
    return input.flatMap(selectorRules);
  }
  if (!input || typeof input !== "object") {
    return [];
  }
  const rules: string[] = [];
  for (const [key, value] of Object.entries(input)) {
    // An array of strings is a declaration with fallback values, as in `utilityBodies`.
    if (
      !value ||
      typeof value !== "object" ||
      statement(key, value) ||
      (Array.isArray(value) && !value.some((entry) => entry && typeof entry === "object")) ||
      key.startsWith("@keyframes ") ||
      key.startsWith("@property ")
    ) {
      continue;
    }
    if (!key.startsWith("@")) {
      rules.push(`${key}{}`);
    }
    rules.push(...selectorRules(value));
  }
  return rules;
}

/**
 * Flattens nested selector rules into utility bodies with one selector block each, which the
 * compiler composes as independent targets.
 *
 * @param input - CSS-in-JS body, or an array of them.
 * @param selector - Selector of the body relative to the utility class, `&` for the class itself.
 * @param conditions - Enclosing at-rule preludes, outermost first.
 * @returns Bodies in source order. Each one is either declarations, possibly inside
 * conditions, for `&`, one rule for another selector, or one `@keyframes` or `@property` rule
 * inside its conditions.
 */
export function utilityBodies(input: unknown, selector = "&", conditions: string[] = []): string[] {
  if (Array.isArray(input)) {
    return input.flatMap((entry) => utilityBodies(entry, selector, conditions));
  }
  if (!input || typeof input !== "object") {
    return [];
  }
  const result: string[] = [];
  let declarations: Record<string, unknown> = {};
  /** Adds the declarations collected so far as one body and starts a new collection. */
  const flush = () => {
    const css = serializeCss(declarations);
    if (css) {
      const body = conditions.reduceRight((body, condition) => `${condition}{${body}}`, css);
      result.push(selector === "&" ? body : `${selector}{${body}}`);
    }
    declarations = {};
  };
  for (const [key, value] of Object.entries(input)) {
    if (
      value &&
      typeof value === "object" &&
      !statement(key, value) &&
      (!Array.isArray(value) || value.some((entry) => entry && typeof entry === "object"))
    ) {
      flush();
      if (key.startsWith("@keyframes ") || key.startsWith("@property ")) {
        result.push(
          conditions.reduceRight(
            (body, condition) => `${condition}{${body}}`,
            `${key}{${serializeCss(value)}}`,
          ),
        );
      } else if (key.startsWith("@")) {
        result.push(...utilityBodies(value, selector, [...conditions, key]));
      } else {
        // Each target keeps one selector, because a utility body holds one selector block.
        for (const parent of splitTopLevel(selector, ",").map((part) => part.trim())) {
          for (const child of splitTopLevel(key, ",").map((part) => part.trim())) {
            result.push(
              ...utilityBodies(
                value,
                substituteNesting(child, parent) ?? `${parent} ${child}`,
                conditions,
              ),
            );
          }
        }
      }
    } else {
      declarations[key] = value;
    }
  }
  flush();
  return result;
}

/**
 * Splits text at a delimiter outside CSS strings, brackets, and function calls.
 *
 * @param value - Text to split. Escaped characters never delimit.
 * @param delimiter - One character, such as `,` or `/`.
 * @returns The parts without trimming, including empty ones. Text without the delimiter is one
 * part.
 */
export function splitTopLevel(value: string, delimiter: string): string[] {
  const parts: string[] = [];
  let start = 0;
  let depth = 0;
  let quote = "";
  for (let index = 0; index < value.length; index++) {
    const char = value[index];
    if (char === "\\") {
      index++;
      continue;
    }
    if (quote) {
      if (char === quote) {
        quote = "";
      }
      continue;
    }
    if (char === '"' || char === "'") {
      quote = char;
      continue;
    }
    if (char === "[" || char === "(") {
      depth++;
    } else if (char === "]" || char === ")") {
      depth--;
    } else if (depth === 0 && char === delimiter) {
      parts.push(value.slice(start, index));
      start = index + 1;
    }
  }
  parts.push(value.slice(start));
  return parts;
}

/**
 * Converts the declarations of a Tailwind `@plugin` options block into the values that
 * `plugin.withOptions()` factories receive.
 *
 * @param options - Declarations in authored order, such as `prefix` with the value `"x"`.
 * @returns Options by name, converted as Tailwind converts them. A later declaration of a
 * name replaces an earlier one. `true`, `false`, `null`, and numeric text become those values.
 * A quoted string loses its quotes and keeps its backslashes. Other text stays a string, and a
 * comma-separated list becomes an array.
 */
export function pluginOptions(options: readonly NativePluginOption[]): Record<string, unknown> {
  const result: Record<string, unknown> = {};
  for (const { name, value: text } of options) {
    const entries = splitTopLevel(text, ",").map((part) => {
      const value = part.trim();
      if (value === "true") {
        return true;
      }
      if (value === "false") {
        return false;
      }
      if (value === "null") {
        return null;
      }
      if (!Number.isNaN(Number(value))) {
        return Number(value);
      }
      return (value.startsWith('"') && value.endsWith('"')) ||
        (value.startsWith("'") && value.endsWith("'"))
        ? value.slice(1, -1)
        : value;
    });
    result[name] = entries.length === 1 ? entries[0] : entries;
  }
  return result;
}
