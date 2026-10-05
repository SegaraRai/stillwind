import { readFileSync, readdirSync } from "node:fs";
import { join } from "node:path";

import { parseMarkdownFixtureCases } from "@stillwind-internal/tools-tailwind/markdown-fixtures";
import {
  propertyRelations,
  type PropertyRelations,
} from "@stillwind-internal/tools-tailwind/property-order";
import { transform } from "lightningcss";

import { compileFixtureSource } from "./native-input.ts";
import { stillwindFixtureCss } from "./stillwind-css.ts";

interface DeclarationBlock {
  declarations?: unknown[];
  importantDeclarations?: unknown[];
}

interface StyleRule {
  declarations?: DeclarationBlock;
  selectors: Array<Array<{ name?: string; type: string }>>;
}

interface ClassRule {
  important: string[];
  normal: string[];
  order: number;
}

interface NativeOutput {
  classes: Array<{ className: string }>;
  css: string;
  diagnostics: unknown[];
}

export interface PropertyOrderParityResult {
  cases: number;
  candidates: number;
  constraints: number;
}

function propertyIdName(id: unknown): string | undefined {
  if (typeof id === "string") {
    return id;
  }
  if (!id || typeof id !== "object") {
    return undefined;
  }
  const property = Reflect.get(id, "property");
  if (typeof property !== "string") {
    return undefined;
  }
  if (property !== "custom") {
    return property;
  }
  const value = Reflect.get(id, "value");
  return value && typeof value === "object" ? String(Reflect.get(value, "name")) : undefined;
}

function declarationName(declaration: unknown): string | undefined {
  if (!declaration || typeof declaration !== "object") {
    return undefined;
  }
  const property = Reflect.get(declaration, "property");
  if (typeof property !== "string") {
    return undefined;
  }
  const value = Reflect.get(declaration, "value");
  if (property === "custom") {
    return value && typeof value === "object" ? String(Reflect.get(value, "name")) : undefined;
  }
  if (property === "unparsed") {
    return value && typeof value === "object"
      ? propertyIdName(Reflect.get(value, "propertyId"))
      : undefined;
  }
  return property;
}

function exactClassName(rule: StyleRule): string | undefined {
  if (rule.selectors.length !== 1) {
    return undefined;
  }
  const selector = rule.selectors[0];
  if (selector.length !== 1 || selector[0].type !== "class") {
    return undefined;
  }
  return selector[0].name;
}

function classRules(css: string, accepted: ReadonlySet<string>): Map<string, ClassRule[]> {
  const rules = new Map<string, ClassRule[]>();
  let order = 0;
  transform({
    code: Buffer.from(css),
    errorRecovery: true,
    filename: "property-order.css",
    visitor: {
      Rule: {
        style(rule) {
          const value = rule.value as StyleRule;
          const className = exactClassName(value);
          if (!className || !accepted.has(className)) {
            return;
          }
          const normal = (value.declarations?.declarations ?? [])
            .map(declarationName)
            .filter((name): name is string => name !== undefined);
          const important = (value.declarations?.importantDeclarations ?? [])
            .map(declarationName)
            .filter((name): name is string => name !== undefined);
          if (normal.length > 0 || important.length > 0) {
            const occurrences = rules.get(className) ?? [];
            occurrences.push({ important, normal, order });
            rules.set(className, occurrences);
            order += 1;
          }
        },
      },
    },
  });
  return rules;
}

function hasVariant(candidate: string): boolean {
  let brackets = 0;
  let parentheses = 0;
  let escaped = false;
  for (const character of candidate) {
    if (escaped) {
      escaped = false;
    } else if (character === "\\") {
      escaped = true;
    } else if (character === "[") {
      brackets += 1;
    } else if (character === "]") {
      brackets -= 1;
    } else if (character === "(") {
      parentheses += 1;
    } else if (character === ")") {
      parentheses -= 1;
    } else if (character === ":" && brackets === 0 && parentheses === 0) {
      return true;
    }
  }
  return false;
}

function lastPositions(properties: string[]): Map<string, number> {
  const positions = new Map<string, number>();
  properties.forEach((property, index) => positions.set(property, index));
  return positions;
}

function compareBucket(
  label: string,
  expected: string[],
  actual: string[],
  relations: PropertyRelations,
): number {
  const expectedPositions = lastPositions(expected);
  const actualPositions = lastPositions(actual);
  let constraints = 0;
  for (const property of expectedPositions.keys()) {
    if (!actualPositions.has(property)) {
      throw new Error(`${label}: Stillwind dropped ${property} while combining utilities`);
    }
  }
  const properties = [...expectedPositions.keys()];
  for (let left = 0; left < properties.length; left += 1) {
    for (let right = left + 1; right < properties.length; right += 1) {
      const leftProperty = properties[left];
      const rightProperty = properties[right];
      if (!relations.interferes(leftProperty, rightProperty)) {
        continue;
      }
      constraints += 1;
      const expectedOrder =
        expectedPositions.get(leftProperty)! < expectedPositions.get(rightProperty)!;
      const actualOrder = actualPositions.get(leftProperty)! < actualPositions.get(rightProperty)!;
      if (expectedOrder !== actualOrder) {
        throw new Error(
          `${label}: interfering properties ${leftProperty} and ${rightProperty} are ordered differently\n` +
            `Tailwind: ${expectedOrder ? leftProperty : rightProperty} before ${expectedOrder ? rightProperty : leftProperty}\n` +
            `Stillwind: ${actualOrder ? leftProperty : rightProperty} before ${actualOrder ? rightProperty : leftProperty}`,
        );
      }
    }
  }
  return constraints;
}

export function verifyPropertyOrderParity(
  fixtureRoot: string,
  options: { fileFilter?: string } = {},
): PropertyOrderParityResult {
  const relations = propertyRelations();
  const caseRoot = join(fixtureRoot, "cases");
  let caseCount = 0;
  let candidateCount = 0;
  let constraintCount = 0;

  for (const file of readdirSync(caseRoot)
    .filter((name) => name.endsWith(".fixture.md"))
    .filter((name) => !options.fileFilter || name === options.fileFilter)
    .sort()) {
    const markdown = readFileSync(join(caseRoot, file), "utf8");
    for (const fixtureCase of parseMarkdownFixtureCases(markdown)) {
      if (/@(?:source|config|plugin|reference)\b/.test(fixtureCase.input)) {
        continue;
      }
      const candidates = fixtureCase.candidates
        .split(/\s+/)
        .filter((candidate) => candidate && !hasVariant(candidate));
      if (candidates.length < 2) {
        continue;
      }
      const expectedCss = markdown.slice(fixtureCase.expectedStart, fixtureCase.expectedEnd);
      const accepted = new Set(candidates);
      const expectedRules = classRules(expectedCss, accepted);
      const applicable = candidates.filter((candidate) => expectedRules.has(candidate));
      if (applicable.length < 2) {
        continue;
      }

      const source = `import { style } from "stillwind/react"; style(${JSON.stringify(applicable.join(" "))});`;
      const native = compileFixtureSource(
        source,
        "src/PropertyOrderParity.tsx",
        stillwindFixtureCss(fixtureCase.input),
      ) as NativeOutput;
      if (native.diagnostics.length > 0 || native.classes.length !== 1) {
        throw new Error(
          `${file} ${fixtureCase.name}: combined compilation failed: ${JSON.stringify(native.diagnostics)}`,
        );
      }
      const generatedClass = native.classes[0].className;
      const actualRules = classRules(native.css, new Set([generatedClass])).get(generatedClass);
      if (!actualRules) {
        throw new Error(`${file} ${fixtureCase.name}: generated base class rule was not found`);
      }

      const orderedExpected = applicable
        .flatMap((candidate) => expectedRules.get(candidate)!)
        .sort((left, right) => left.order - right.order);
      const orderedActual = actualRules.sort((left, right) => left.order - right.order);
      const label = `${file} ${fixtureCase.name}`;
      constraintCount += compareBucket(
        `${label} (normal)`,
        orderedExpected.flatMap((rule) => rule.normal),
        orderedActual.flatMap((rule) => rule.normal),
        relations,
      );
      constraintCount += compareBucket(
        `${label} (!important)`,
        orderedExpected.flatMap((rule) => rule.important),
        orderedActual.flatMap((rule) => rule.important),
        relations,
      );
      caseCount += 1;
      candidateCount += applicable.length;
    }
  }

  return {
    candidates: candidateCount,
    cases: caseCount,
    constraints: constraintCount,
  };
}
