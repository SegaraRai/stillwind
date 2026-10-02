import webref from "@webref/css/css.json" with { type: "json" };
import { generate, parse, walk, type Block, type CssNode, type Declaration } from "css-tree";

interface WebrefProperty {
  name: string;
  legacyAliasOf?: string;
  logicalPropertyGroup?: string;
  longhands?: string[];
  resetLonghands?: string[];
}

interface WebrefCss {
  properties: WebrefProperty[];
}

function isLogicalProperty(name: string): boolean {
  return (
    /(?:^|-)(?:block|inline)(?:-|$)/.test(name) ||
    /(?:border|corner)-(?:start|end)-(?:start|end)(?:-|$)/.test(name)
  );
}

export class PropertyRelations {
  readonly #affected = new Map<string, Set<string>>();
  readonly #properties: Map<string, WebrefProperty>;

  constructor(properties: WebrefProperty[]) {
    this.#properties = new Map(properties.map((property) => [property.name, property]));
  }

  affected(name: string, visiting = new Set<string>()): Set<string> {
    const cached = this.#affected.get(name);
    if (cached) {
      return cached;
    }
    if (name === "all") {
      return new Set(["*"]);
    }
    if (visiting.has(name)) {
      throw new Error(`cyclic Webref longhand relation for ${name}`);
    }
    const property = this.#properties.get(name);
    const canonical = property?.legacyAliasOf ?? name;
    const children = [...(property?.longhands ?? []), ...(property?.resetLonghands ?? [])];
    if (children.length === 0) {
      const result = new Set([canonical]);
      this.#affected.set(name, result);
      return result;
    }
    const next = new Set(visiting).add(name);
    const result = new Set(children.flatMap((child) => [...this.affected(child, next)]));
    this.#affected.set(name, result);
    return result;
  }

  interferes(left: string, right: string): boolean {
    if (left === right) {
      return false;
    }
    const leftAffected = this.affected(left);
    const rightAffected = this.affected(right);
    if (leftAffected.has("*") || rightAffected.has("*")) {
      return !left.startsWith("--") && !right.startsWith("--");
    }
    if ([...leftAffected].some((property) => rightAffected.has(property))) {
      return true;
    }

    for (const leftProperty of leftAffected) {
      const leftDefinition = this.#properties.get(leftProperty);
      const group = leftDefinition?.logicalPropertyGroup;
      if (!group) {
        continue;
      }
      for (const rightProperty of rightAffected) {
        const rightDefinition = this.#properties.get(rightProperty);
        if (
          rightDefinition?.logicalPropertyGroup === group &&
          isLogicalProperty(leftProperty) !== isLogicalProperty(rightProperty)
        ) {
          return true;
        }
      }
    }
    return false;
  }
}

let cachedPropertyRelations: PropertyRelations | undefined;

export function propertyRelations(): PropertyRelations {
  cachedPropertyRelations ??= new PropertyRelations((webref as WebrefCss).properties);
  return cachedPropertyRelations;
}

interface OrderedDeclaration {
  key: string;
  name: string;
}

interface OrderedStyleRule {
  important: OrderedDeclaration[];
  normal: OrderedDeclaration[];
}

function treeDeclarationKey(declaration: Declaration): string {
  return generate(declaration);
}

function canonicalizeDeclarationOrder(css: string): {
  css: string;
  rules: OrderedStyleRule[];
} {
  const rules: OrderedStyleRule[] = [];
  const ast = parse(css);
  walk(ast, {
    visit: "Block",
    enter(block: Block) {
      const children = block.children.toArray();
      const canonical: CssNode[] = [];
      for (let index = 0; index < children.length;) {
        if (children[index].type !== "Declaration") {
          canonical.push(children[index]);
          index += 1;
          continue;
        }
        const declarations: Declaration[] = [];
        while (index < children.length && children[index].type === "Declaration") {
          declarations.push(children[index] as Declaration);
          index += 1;
        }
        const ordered = declarations.map((declaration) => ({
          key: treeDeclarationKey(declaration),
          name: declaration.property,
        }));
        rules.push({
          normal: ordered.filter(
            (_, declarationIndex) => !declarations[declarationIndex].important,
          ),
          important: ordered.filter(
            (_, declarationIndex) => declarations[declarationIndex].important,
          ),
        });
        canonical.push(
          ...declarations.sort((left, right) =>
            `${left.important}:${treeDeclarationKey(left)}`.localeCompare(
              `${right.important}:${treeDeclarationKey(right)}`,
            ),
          ),
        );
      }
      block.children.fromArray(canonical);
    },
  });
  return {
    css: generate(ast),
    rules,
  };
}

function preservesDependentDeclarationOrder(
  expected: OrderedDeclaration[],
  actual: OrderedDeclaration[],
  relations: PropertyRelations,
): boolean {
  const properties = [...new Set(expected.map((declaration) => declaration.name))];
  for (let left = 0; left < properties.length; left += 1) {
    for (let right = left; right < properties.length; right += 1) {
      const leftProperty = properties[left];
      const rightProperty = properties[right];
      if (leftProperty !== rightProperty && !relations.interferes(leftProperty, rightProperty)) {
        continue;
      }
      const includesProperty = (declaration: OrderedDeclaration) =>
        declaration.name === leftProperty || declaration.name === rightProperty;
      const expectedOrder = expected.filter(includesProperty).map(({ key }) => key);
      const actualOrder = actual.filter(includesProperty).map(({ key }) => key);
      if (expectedOrder.join("\0") !== actualOrder.join("\0")) {
        return false;
      }
    }
  }
  return true;
}

/**
 * Treats declaration order as equivalent exactly when CSS property relations
 * say the declarations commute. All other CSS structure and values must still
 * serialize identically.
 */
export function hasEquivalentDeclarationOrder(expected: string, actual: string): boolean {
  const relations = propertyRelations();
  const canonicalExpected = canonicalizeDeclarationOrder(expected);
  const canonicalActual = canonicalizeDeclarationOrder(actual);
  if (
    canonicalExpected.css !== canonicalActual.css ||
    canonicalExpected.rules.length !== canonicalActual.rules.length
  ) {
    return false;
  }
  return canonicalExpected.rules.every((expectedRule, index) => {
    const actualRule = canonicalActual.rules[index];
    return (
      preservesDependentDeclarationOrder(expectedRule.normal, actualRule.normal, relations) &&
      preservesDependentDeclarationOrder(expectedRule.important, actualRule.important, relations)
    );
  });
}
