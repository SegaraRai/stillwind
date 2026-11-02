/**
 * Classifies the landing page code specimens into syntax token kinds.
 * The tokenizers cover only the constructs that the specimens use.
 * They are not general JavaScript or CSS parsers.
 */

/**
 * Describes the role of a token. Whitespace uses `punctuation`.
 * `api` covers Stillwind calls, `variant` covers variant names in keys and utility prefixes,
 * and `string` covers string content that is not a utility.
 */
export type SyntaxKind =
  | "api"
  | "variant"
  | "utility"
  | "identifier"
  | "property"
  | "string"
  | "keyword"
  | "punctuation";

/**
 * Holds token text and its UTF-16 offset in the source. Tokens cover the source in order.
 */
export interface SyntaxToken {
  text: string;
  kind: SyntaxKind;
  start: number;
}

const scriptPattern = /"(?:[^"\\]|\\.)*"|[A-Za-z_$][\w$]*|\d+(?:\.\d+)?|&&|\|\||\s+|./g;

const keywords = new Set(["const", "let", "return", "true", "false", "null", "&&", "||"]);

const apiMembers = new Set([
  "ref",
  "at",
  "asContainer",
  "in",
  "asAnchor",
  "positionAnchor",
  "keyframes",
  "viewTransition",
  "for",
]);

/** Callees whose string arguments, array items, and object values are utilities. */
const utilityCallees = new Set(["style", "style.keyframes", "style.viewTransition"]);

/**
 * Splits utility text at spaces and at colons outside brackets.
 * Segments before a colon are variants, and the final segment of each word is a utility.
 */
function utilityTokens(text: string, start: number, tokens: SyntaxToken[]) {
  for (const match of text.matchAll(/\s+|\S+/g)) {
    const word = match[0];
    const offset = start + match.index;
    if (!word.trim()) {
      tokens.push({
        text: word,
        kind: "punctuation",
        start: offset,
      });
      continue;
    }
    let depth = 0;
    let segment = 0;
    for (let index = 0; index <= word.length; index++) {
      const char = word[index];
      if (char === "[" || char === "(") {
        depth++;
      } else if (char === "]" || char === ")") {
        depth--;
      } else if ((char === ":" && depth === 0) || index === word.length) {
        if (index > segment) {
          tokens.push({
            text: word.slice(segment, index),
            kind: char === ":" ? "variant" : "utility",
            start: offset + segment,
          });
        }
        if (char === ":") {
          tokens.push({
            text: ":",
            kind: "punctuation",
            start: offset + index,
          });
        }
        segment = index + 1;
      }
    }
  }
}

/**
 * Tokenizes a JavaScript expression or a JSON object.
 * Strings are utilities when they are direct arguments of `style()` or of a `style.keyframes()`
 * result, or when they are array items or object values inside such a call.
 * Other strings are `string` content. An object key that ends in a colon is a variant key.
 */
export function highlightScript(source: string): SyntaxToken[] {
  const tokens: SyntaxToken[] = [];
  const contexts: {
    closer: string;
    utilities: boolean;
    object: boolean;
  }[] = [];
  const utilityFunctions = new Set<string>();
  const significant: string[] = [];
  let declared: string | undefined;
  for (const match of source.matchAll(scriptPattern)) {
    const text = match[0];
    const start = match.index;
    const context = contexts.at(-1);
    const rest = source.slice(start + text.length);
    const isKey = !!context?.object && /^\s*:/.test(rest);
    if (!text.trim()) {
      tokens.push({
        text,
        kind: "punctuation",
        start,
      });
      continue;
    }
    if (text.startsWith('"')) {
      const content = text.slice(1, -1);
      tokens.push({
        text: '"',
        kind: "punctuation",
        start,
      });
      if (isKey && content.endsWith(":")) {
        utilityTokens(content, start + 1, tokens);
      } else if (isKey) {
        tokens.push({
          text: content,
          kind: "property",
          start: start + 1,
        });
      } else if (context?.utilities) {
        utilityTokens(content, start + 1, tokens);
      } else if (content) {
        tokens.push({
          text: content,
          kind: "string",
          start: start + 1,
        });
      }
      tokens.push({
        text: '"',
        kind: "punctuation",
        start: start + text.length - 1,
      });
    } else if (/^[A-Za-z_$]/.test(text)) {
      const member = significant.at(-1) === ".";
      let kind: SyntaxKind = "identifier";
      if (isKey) {
        kind = "property";
      } else if (member ? apiMembers.has(text) : text === "style") {
        kind = "api";
      } else if (keywords.has(text) && !member) {
        kind = "keyword";
      }
      if (significant.at(-1) === "const") {
        declared = text;
      }
      tokens.push({
        text,
        kind,
        start,
      });
    } else if (keywords.has(text)) {
      tokens.push({
        text,
        kind: "keyword",
        start,
      });
    } else if (/^\d/.test(text)) {
      tokens.push({
        text,
        kind: "identifier",
        start,
      });
    } else {
      if (text === "(") {
        const name = significant.at(-1) ?? "";
        const callee = significant.at(-2) === "." ? significant.at(-3) + "." + name : name;
        if (callee === "style.keyframes" && declared) {
          utilityFunctions.add(declared);
        }
        contexts.push({
          closer: ")",
          utilities: utilityCallees.has(callee) || utilityFunctions.has(callee),
          object: false,
        });
      } else if (text === "[" || text === "{") {
        contexts.push({
          closer: text === "[" ? "]" : "}",
          utilities: !!context?.utilities,
          object: text === "{",
        });
      } else if (text === context?.closer) {
        contexts.pop();
      }
      tokens.push({
        text,
        kind: "punctuation",
        start,
      });
    }
    significant.push(text);
  }
  return tokens;
}

const cssPattern = /@[\w-]+|--[\w-]+|\.[\w-]+|[A-Za-z_][\w-]*|\d+(?:\.\d+)?[a-z%]*|\s+|./g;

/**
 * Tokenizes formatted CSS. Class selectors are `string` content, so that they match generated
 * class names in JavaScript output. Property names are `property`, at-rules and CSS functions
 * are `keyword`, and other values are `identifier`.
 */
export function highlightCss(source: string): SyntaxToken[] {
  const tokens: SyntaxToken[] = [];
  let depth = 0;
  let value = false;
  for (const match of source.matchAll(cssPattern)) {
    const text = match[0];
    const start = match.index;
    let kind: SyntaxKind = "punctuation";
    if (!/^(?:[\w@]|[.-][\w-])/.test(text)) {
      if (text === "{") {
        depth++;
        value = false;
      } else if (text === "}") {
        depth--;
        value = false;
      } else if (text === ";") {
        value = false;
      } else if (text === ":" && depth > 0) {
        value = true;
      }
    } else if (text.startsWith("@")) {
      kind = "keyword";
    } else if (depth > 0 && !value) {
      kind = "property";
    } else if (depth === 0 && text.startsWith(".")) {
      kind = "string";
    } else if (value && source[start + text.length] === "(") {
      kind = "keyword";
    } else {
      kind = "identifier";
    }
    tokens.push({
      text,
      kind,
      start,
    });
  }
  return tokens;
}
