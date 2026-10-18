import path from "node:path";

import { ResolverFactory } from "oxc-resolver";

/**
 * Creates a resolver for stylesheet imports. It selects `.css` files, the `style` export
 * condition, and the `style` package field, as CSS tooling resolves packages such as
 * `tw-animate-css`.
 *
 * @returns A new resolver with its own cache.
 */
export function stylesheetResolver(): ResolverFactory {
  return new ResolverFactory({
    extensions: [".css"],
    conditionNames: ["style", "import", "default"],
    mainFields: ["style", "main"],
  });
}

/**
 * Creates a stylesheet package resolver for hosts whose own resolver selects JavaScript entries.
 *
 * @returns A function that resolves a specifier from an importing file to a path, or to
 * undefined when the specifier does not resolve.
 */
export function createStylesheetResolver(): (
  specifier: string,
  importer: string,
) => Promise<string | undefined> {
  const resolver = stylesheetResolver();
  return async (specifier, importer) =>
    (await resolver.async(path.dirname(importer), specifier)).path;
}
