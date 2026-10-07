import type { NativeAssetRecord } from "@stillwind/native";

import type { ModuleAnalysis } from "../compilation/compiler.ts";

/** Collects emitted rules owned by a style and its composed bases, preserving other selectors. */
export function stylePreview(graph: ModuleAnalysis, asset: NativeAssetRecord): string | undefined {
  const identities = [asset.identity];
  for (let index = 0; index < identities.length; index++) {
    for (const module of graph.modules) {
      for (const edge of module.composition) {
        if (
          same(edge.derived, identities[index]) &&
          !identities.some((identity) => same(identity, edge.base))
        ) {
          identities.push(edge.base);
        }
      }
    }
  }

  const names = new Set<string>();
  for (const identity of identities) {
    const module = graph.modules.find((module) => module.moduleId === identity.moduleId);
    for (const generated of module?.classes ?? []) {
      if (generated.callSpan.start === identity.start && generated.callSpan.end === identity.end) {
        names.add(generated.className);
      }
    }
  }
  if (!names.size) {
    return undefined;
  }

  const rules = graph.modules.map((module) => selectRules(module.css, names)).filter(Boolean);
  return rules.join("\n") || undefined;
}

function same(left: NativeAssetRecord["identity"], right: NativeAssetRecord["identity"]): boolean {
  return (
    left.moduleId === right.moduleId &&
    left.start === right.start &&
    left.end === right.end &&
    left.kind === right.kind
  );
}

/** Reads block boundaries without treating quoted braces or comments as CSS syntax. */
function selectRules(css: string, names: ReadonlySet<string>): string {
  let start = 0;
  let depth = 0;
  let opening = -1;
  let quote = "";
  let comment = false;
  let result = "";
  for (let index = 0; index < css.length; index++) {
    const character = css[index];
    if (comment) {
      if (css.slice(index, index + 2) === "*/") {
        comment = false;
        index++;
      }
      continue;
    }
    if (quote) {
      if (character === "\\") {
        index++;
      } else if (character === quote) {
        quote = "";
      }
      continue;
    }
    if (css.slice(index, index + 2) === "/*") {
      comment = true;
      index++;
      continue;
    }
    if (character === '"' || character === "'") {
      quote = character;
    } else if (character === "{") {
      if (depth++ === 0) {
        opening = index;
      }
    } else if (character === "}" && --depth === 0) {
      let prelude = css.slice(start, opening).trim();
      const body = css.slice(opening + 1, index);
      if (prelude.startsWith("@") && !/^@(?:keyframes|property|font-face)\b/.test(prelude)) {
        const nested = selectRules(body, names);
        if (nested) {
          result += `${prelude} {\n${nested}}\n`;
        }
      } else {
        const selected = replaceClasses(prelude, names);
        prelude = selected.selector;
        const owned = selected.owned;
        if (owned) {
          result += `${prelude} {${body}}\n`;
        }
      }
      start = index + 1;
    } else if (character === ";" && depth === 0) {
      start = index + 1;
    }
  }
  return result;
}

function replaceClasses(
  selector: string,
  names: ReadonlySet<string>,
): { selector: string; owned: boolean } {
  let result = "";
  let owned = false;
  let quote = "";
  for (let index = 0; index < selector.length; index++) {
    const character = selector[index];
    if (character === "\\") {
      result += selector.slice(index, index + 2);
      index++;
      continue;
    }
    if (quote) {
      result += character;
      if (character === quote) {
        quote = "";
      }
      continue;
    }
    if (character === '"' || character === "'") {
      quote = character;
    }
    if (selector.slice(index, index + 2) === "/*") {
      const end = selector.indexOf("*/", index + 2);
      if (end < 0) {
        result += selector.slice(index);
        break;
      }
      result += selector.slice(index, end + 2);
      index = end + 1;
      continue;
    }
    if (character === ".") {
      const spelling = /^(?:\\[\da-f]{1,6}\s?|\\[^\r\n\f]|[\w\u0080-\uffff-])+/iu.exec(
        selector.slice(index + 1),
      )?.[0];
      if (spelling) {
        const name = spelling.replace(/\\([\da-f]{1,6}\s?|.)/giu, (_, escape: string) => {
          const point = /^[\da-f]/i.test(escape) ? Number.parseInt(escape.trim(), 16) : undefined;
          return point === undefined
            ? escape
            : String.fromCodePoint(
                point > 0 && point <= 0x10ffff && !(point >= 0xd800 && point <= 0xdfff)
                  ? point
                  : 0xfffd,
              );
        });
        const match = names.has(name);
        result += match ? "&" : `.${spelling}`;
        owned ||= match;
        index += spelling.length;
        continue;
      }
    }
    result += character;
  }
  return {
    selector: result,
    owned,
  };
}
