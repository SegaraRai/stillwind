import path from "node:path";

import { describe, expect, test } from "vite-plus/test";

import { ModuleRegistry } from "./module-registry.ts";

const projectRoot = path.resolve("registry-fixture");
const styleFile = path.join(projectRoot, "style.ts");

describe("CSS value references", () => {
  test("different queries on the same file receive different references", () => {
    const registry = new ModuleRegistry(projectRoot, "client");
    const firstSource = `${styleFile}?pick=first`;
    const secondSource = `${styleFile}?pick=second`;

    const firstReference = registry.source("value", firstSource);
    const secondReference = registry.source("value", secondSource);

    expect(firstReference).not.toBe(secondReference);
    expect(registry.get(firstReference)).toMatchObject({ sourceId: firstSource });
    expect(registry.get(secondReference)).toMatchObject({ sourceId: secondSource });
  });

  test("SSR cannot use a reference issued by the client registry", () => {
    const clientRegistry = new ModuleRegistry(projectRoot, "client");
    const ssrRegistry = new ModuleRegistry(projectRoot, "ssr");
    const reference = clientRegistry.source("value", styleFile);

    expect(clientRegistry.get(reference)).toBeDefined();
    expect(ssrRegistry.get(reference)).toBeUndefined();
  });

  test("encoding a file path does not register a reference", () => {
    const registry = new ModuleRegistry(projectRoot, "client");
    const encodedPath = Buffer.from(styleFile).toString("base64url");
    const forgedReference = `virtual:stillwind/value:${encodedPath}`;

    expect(registry.get(forgedReference)).toBeUndefined();
  });

  test("removing a file invalidates references for all its queries", () => {
    const registry = new ModuleRegistry(projectRoot, "client");
    const firstReference = registry.source("value", `${styleFile}?pick=first`);
    const secondReference = registry.source("value", `${styleFile}?pick=second`);

    registry.remove(styleFile, true);

    expect(registry.get(firstReference)).toBeUndefined();
    expect(registry.get(secondReference)).toBeUndefined();
  });
});

describe("imports of server CSS", () => {
  const importer = path.join(projectRoot, "main.ts");
  const target = path.join(projectRoot, "server.ts");
  const specifier = "./server.ts?stillwind.css";

  test("a registered import resolves only for its original importer", () => {
    const registry = new ModuleRegistry(projectRoot, "client");
    registry.projections(importer, new Map([[specifier, target]]));

    const reference = registry.projection(specifier, importer);

    expect(reference).toBeDefined();
    expect(registry.get(reference!)).toMatchObject({ kind: "projection", sourceId: target });
    expect(registry.projection(specifier)).toBeUndefined();
    expect(registry.projection(specifier, path.join(projectRoot, "other.ts"))).toBeUndefined();
    expect(registry.projection("./other.ts?stillwind.css", importer)).toBeUndefined();
  });

  test("removing an import invalidates its CSS reference", () => {
    const registry = new ModuleRegistry(projectRoot, "client");
    registry.projections(importer, new Map([[specifier, target]]));
    const reference = registry.projection(specifier, importer)!;

    registry.projections(importer, new Map());

    expect(registry.projection(specifier, importer)).toBeUndefined();
    expect(registry.get(reference)).toBeUndefined();
  });

  test("deleting the imported file invalidates its CSS reference", () => {
    const registry = new ModuleRegistry(projectRoot, "client");
    registry.projections(importer, new Map([[specifier, target]]));
    const reference = registry.projection(specifier, importer)!;

    registry.remove(target);

    expect(registry.projection(specifier, importer)).toBeUndefined();
    expect(registry.get(reference)).toBeUndefined();
  });
});

describe("CSS fragments with image resources", () => {
  const css = 'a{background:url("IMAGE_PLACEHOLDER")}';
  /**
   * Describes an image resource for fragment identity tests.
   * @param name - Image path relative to the fixture project root.
   * @returns A resource whose resolver identity and readable path refer to the same file.
   */
  const image = (name: string) => {
    const file = path.join(projectRoot, name);
    return { id: file, file };
  };

  test("changing the image changes the fragment reference", () => {
    const registry = new ModuleRegistry(projectRoot, "client");

    const firstReference = registry.fragment(
      css,
      new Map([["IMAGE_PLACEHOLDER", image("first.svg")]]),
    );
    const secondReference = registry.fragment(
      css,
      new Map([["IMAGE_PLACEHOLDER", image("second.svg")]]),
    );

    expect(secondReference).not.toBe(firstReference);
  });

  test("adding an image that the CSS does not use keeps the same reference", () => {
    const registry = new ModuleRegistry(projectRoot, "client");
    const usedImage = image("icon.svg");
    const reference = registry.fragment(css, new Map([["IMAGE_PLACEHOLDER", usedImage]]));

    const referenceWithUnusedImage = registry.fragment(
      css,
      new Map([
        ["IMAGE_PLACEHOLDER", usedImage],
        ["UNUSED_PLACEHOLDER", image("unused.svg")],
      ]),
    );

    expect(referenceWithUnusedImage).toBe(reference);
  });

  test("clearing the registry invalidates its fragment references", () => {
    const registry = new ModuleRegistry(projectRoot, "client");
    const reference = registry.fragment(css, new Map([["IMAGE_PLACEHOLDER", image("icon.svg")]]));

    registry.clear();

    expect(registry.get(reference)).toBeUndefined();
  });
});
