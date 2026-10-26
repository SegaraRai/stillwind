import { readFile } from "node:fs/promises";

import { expect, test } from "vite-plus/test";

import { completionLabels } from "./support.ts";

/**
 * Registers candidate query tests for the selected native or WASM binding.
 *
 * @param specifier - Absolute URL of the binding module.
 */
export function registerCandidatesEditorContract(specifier: string): void {
  test("shared assets stay outside token queries and remain available to CSS references", async () => {
    const { NativeDesignSystemLoader } = await import(specifier);
    const loader = new NativeDesignSystemLoader();
    const definitions = `@stillwind-theme-namespace --image-* [image];
@theme default asset {/* 🦀 */--image-data:linear-gradient(red,blue);}
@theme {--*:initial; --image-token:linear-gradient(green,blue);}
@theme asset static {--retained:url(data:image/svg+xml,retained);}
@theme asset inline {--inline:url(data:image/svg+xml,inline);}
@utility paint-* {background-image:--value(--image-*, [image]);}
@utility icon {mask-image:var(--image-data);}`;
    loader.provide(0, "theme.css", definitions, "file:///workspace/theme.css");
    const query = loader.editorQuery();
    expect(query.sharedAssets()).toEqual([
      {
        name: "--image-data",
        value: "linear-gradient(red,blue)",
        fallback: true,
        mode: "default",
      },
      {
        name: "--inline",
        value: "url(data:image/svg+xml,inline)",
        fallback: false,
        mode: "inline",
      },
      {
        name: "--retained",
        value: "url(data:image/svg+xml,retained)",
        fallback: false,
        mode: "static",
      },
    ]);
    expect(query.themeTokens().map((token: { name: string }) => token.name)).toEqual([
      "--image-token",
    ]);
    expect(query.themeValues()).toEqual({ "--image-token": "linear-gradient(green,blue)" });
    expect(completionLabels(query, "paint-")).toEqual(["paint-token"]);
    expect(query.previewCandidate("paint-data")).toBeNull();
    expect(query.canonicalCandidate("paint-[linear-gradient(red,blue)]")).toBeNull();
    expect(query.canonicalCandidate("paint-[linear-gradient(green,blue)]")).toBe("paint-token");
    for (const [fn, expected] of [
      ["var", ["--image-data", "--image-token"]],
      ["--theme", ["--image-token"]],
    ] as const) {
      const css = `.a {mask-image:${fn}(--image-`;
      expect(
        query.completeStylesheet(css, css.length).map((item: { label: string }) => item.label),
      ).toEqual(expected);
    }
    const css = ".a {mask-image:var(--image-data";
    expect(query.hoverStylesheet(css, css.length)).toMatchObject({
      css: "--image-data: linear-gradient(red,blue);",
      documentation: expect.stringContaining("shared asset"),
    });
    expect(query.sharedAssets()).toHaveLength(3);
    expect(query.stylesheetDefinition(css, css.length)).toEqual({
      uri: "file:///workspace/theme.css",
      startLine: 1,
      endLine: 1,
      startColumn: definitions.split("\n")[1].indexOf("--image-data"),
      endColumn: definitions.split("\n")[1].indexOf("--image-data") + "--image-data".length,
    });
    const themeReference = ".a {mask-image:--theme(--image-data";
    expect(query.stylesheetDefinition(themeReference, themeReference.length)).toBeNull();
  });

  test("marker classes complete, describe their origins, and sort before utilities", async () => {
    const { NativeDesignSystemLoader } = await import(specifier);
    const loader = new NativeDesignSystemLoader();
    loader.provide(
      0,
      "theme.css",
      `@stillwind-marker menu (modifiable);
@utility card-title { .card & { color: red; } }
@utility box { color: blue; }`,
    );
    const query = loader.editorQuery();
    expect(query.completeCandidate("card")).toEqual([
      { label: "card", marker: true },
      { label: "card-title", marker: false },
    ]);
    expect(query.completeCandidate("menu", "utility", true)).toEqual([
      { label: "menu", marker: true },
      { label: "menu/", marker: true },
    ]);
    expect(completionLabels(query, "hover:card")).not.toContain("hover:card");
    expect(query.addedMarker("card")).toEqual({
      class: "card",
      origins: [{ kind: "utility", name: "card-title" }],
    });
    expect(query.addedMarker("menu/main")).toEqual({
      class: "menu/main",
      origins: [{ kind: "declared" }],
    });
    expect(query.addedMarker("hover:card")).toBeNull();
    expect(query.addedMarker("card", "document")).toBeNull();
    expect(query.candidateRanks(["box", "card", "menu", "hover:card"])).toEqual([1, 0, 0, null]);
  });

  test("functional registrations preserve modifiers, nested targets, theme defaults, and CSS functions", async () => {
    const { NativeDesignSystemLoader, NativeProjectSession, decodeArbitraryValue } = await import(
      specifier
    );
    const loader = new NativeDesignSystemLoader();
    loader.provide(
      0,
      "theme.css",
      `
@stillwind-theme-namespace --spacing-* [length] scale;
@theme { --spacing: .25rem; --color-brand: blue; }
@theme default { --color-brand: red; }
@utility duration-* { transition-duration: --value(integer)ms; }
@stillwind-plugin-family utility duration;
@stillwind-plugin-family utility probe;
@stillwind-plugin-family variant state;
@utility duration-300 (functional) { animation-duration: 300ms; }
@utility -probe-\\[3px\\]\\/mirror (functional) { width: calc(3px * -1); }
@utility -probe-\\[3px\\]\\/mirror (functional) { &:hover { color: blue; } }
@utility safe { padding-top: --spacing(4); color: --alpha(red / 50%); }
`,
    );
    const query = loader.editorQuery();
    expect(query.themeTokens()).toContainEqual({
      name: "--color-brand",
      value: "blue",
      fallback: false,
      inline: false,
    });
    expect(query.dynamicCandidates('"duration-300 -probe-[3px]/mirror state-[open]:safe"')).toEqual(
      ["-probe-[3px]/mirror", "@variant state-[open]", "duration-300"],
    );
    expect(
      query.stylesheetDynamicCandidates(
        "@utility duration-* {} /* @apply probe-1; */ .a { @apply duration-300; @variant state-[open] { color: red; } }",
      ),
    ).toEqual(["@variant state-[open]", "duration-300"]);
    // A plugin's functional definition adds to the stylesheet's `duration-*` and comes first,
    // because Tailwind registers plugins before the stylesheet's utilities.
    expect(query.previewCandidate("duration-300")).toBe(
      "&{animation-duration: 300ms;transition-duration: 300ms;}",
    );
    const preview = query.previewCandidate("-probe-[3px]/mirror");
    expect(preview).toBe("&{width: calc(3px * -1);}&:hover{color: blue;}");
    expect(query.previewCandidate("safe")).toContain("var(--spacing)");
    expect(query.previewCandidate("safe")).not.toContain("--spacing(");
    expect(query.previewCandidate("safe")).not.toContain("--alpha(");
    expect(decodeArbitraryValue("calc(var(--my_width)+1px)")).toBe("calc(var(--my_width) + 1px)");
    const session = new NativeProjectSession(loader, false);
    const reload = loader.newLoader();
    reload.provide(0, "theme.css", "@theme { --color-brand: green; }");
    expect(() => session.reload(reload)).not.toThrow();
  });

  test("completion placeholders, summaries, and plugin ranges cross the binding", async () => {
    const { NativeDesignSystemLoader, completionDetail, stylesheetPluginPaths } = await import(
      specifier
    );
    const loader = new NativeDesignSystemLoader();
    loader.provide(
      0,
      "theme.css",
      `@variant hover { &:hover { @slot; } }
@stillwind-variant nth-* (value: [nth]) { &:--value(":nth-child({value})") { @slot; } }`,
    );
    const query = loader.editorQuery();
    expect(completionLabels(query, "hover:", "utility", true)).toContain("hover:nth-[]:");
    expect(completionLabels(query, "hover:")).not.toContain("hover:hover:");
    expect(completionLabels(query, "nth-")).not.toContain("nth-[]:");
    expect(query.previewVariant("hover:", "mediaVariants")).toBeNull();
    expect(completionDetail(query.previewVariant("hover:"))).toBe("&:hover");
    const css = '/* 日本語 🦀 */ @plugin "./loc';
    const bytes = new TextEncoder().encode(css);
    const paths = stylesheetPluginPaths(css, bytes.length);
    expect(paths).toHaveLength(1);
    expect(paths[0]).toMatchObject({
      specifier: "./loc",
      prefix: "./loc",
      quote: '"',
      closed: false,
    });
    expect(new TextDecoder().decode(bytes.slice(paths[0].start, paths[0].end))).toBe("./loc");
  });

  test("candidate colors require color utilities and keep stylesheet reference colors", async () => {
    const { NativeDesignSystemLoader } = await import(specifier);
    const loader = new NativeDesignSystemLoader();
    loader.provide(
      0,
      "theme.css",
      `@theme { --color-brand: red; }
@utility card { background-color: red; padding: 1rem; }
@utility ring-brand { --ring-color: var(--color-brand); }
@utility faded { color: rgb(255 0 0 / 0); outline-color: red; }`,
    );
    const query = loader.editorQuery();
    expect(query.candidateColor("card")).toBeNull();
    expect(query.candidateColor("ring-brand")).toEqual([1, 0, 0, 1]);
    expect(query.candidateColor("faded")).toEqual([1, 0, 0, 1]);
    const css = ".card { @apply card; color: var(--color-brand); }";
    const colors = query.stylesheetColors(css);
    expect(colors).toHaveLength(1);
    expect(css.slice(colors[0].start, colors[0].end)).toBe("var(--color-brand)");
  });

  test("editor input roles share compiler registries, declaration mapping, and variant restrictions", async () => {
    const { NativeDesignSystemLoader } = await import(specifier);
    const motion = [
      await readFile(
        new URL("../../../core/css/builtin-utilities/tw-motion.css", import.meta.url),
        "utf8",
      ),
      await readFile(
        new URL("../../../core/css/builtin-utilities/sw-motion-contexts.css", import.meta.url),
        "utf8",
      ),
    ].join("\n");
    const loader = new NativeDesignSystemLoader();
    loader.provide(
      0,
      "theme.css",
      `${motion}
@stillwind-theme-namespace --duration-* [time];
@stillwind-theme-namespace --ease-* [easing];
@theme { --duration-normal:180ms; --ease-soft:ease-in; }
@utility block { display:block; }
@stillwind-variant hover { &:hover { @slot; } }
@stillwind-variant media { @media print { @slot; } }
@stillwind-variant supports { @supports (display:grid) { @slot; } }
@stillwind-variant container { @container (width>20rem) { @slot; } }
@stillwind-variant mixed { @media print { &:hover { @slot; } } }
@stillwind-variant sequence { @media print { @supports (display:grid) { @slot; } } }`,
    );
    const query = loader.editorQuery();
    expect(query.previewCandidate("duration-180", "utility")).toContain("transition-duration:");
    expect(query.previewCandidate("duration-180", "keyframesFrame")).toBeNull();
    expect(query.previewCandidate("duration-normal", "animationOptions")).toBe(
      "&{animation-duration:var(--duration-normal);}",
    );
    expect(query.previewCandidate("[animation-duration:75ms]", "animationOptions")).toBe(
      "&{animation-duration:75ms;}",
    );
    expect(query.previewCandidate("[transition-duration:75ms]", "animationOptions")).toBeNull();
    expect(completionLabels(query, "duration-no", "animationOptions")).toEqual(["duration-normal"]);
    expect(completionLabels(query, "ease-(--ease-so", "animationOptions")).toContain(
      "ease-(--ease-soft)",
    );
    for (const candidate of ["hover:block", "media:block"]) {
      expect(query.previewCandidate(candidate, "keyframesFrame")).toBeNull();
    }
    for (const candidate of ["block", "media:both", "!both", "both!", "-duration-180"]) {
      expect(query.previewCandidate(candidate, "animationOptions")).toBeNull();
    }
    for (const context of ["animationOptions", "keyframesFrame"]) {
      expect(
        completionLabels(query, "", context).some((label: string) => label.endsWith(":")),
      ).toBe(false);
    }
    for (const context of ["viewTransitionUtilities", "definitionVariants"]) {
      const labels = completionLabels(query, "", context);
      expect(labels).toEqual(expect.arrayContaining(["media:", "supports:", "sequence:"]));
      for (const label of ["hover:", "container:", "mixed:"]) {
        expect(labels).not.toContain(label);
      }
    }
    const both = query.previewCandidate("media:both", "viewTransitionUtilities");
    expect(both).toContain("@media print");
    expect(both).toContain("animation-fill-mode:both;");
    expect(query.previewCandidate("sequence:block", "viewTransitionUtilities")).toContain(
      "@supports (display:grid)",
    );
    expect(query.previewCandidate("hover:block", "viewTransitionUtilities")).toBeNull();
    expect(
      completionLabels(query, "", "definitionVariants").every((label: string) =>
        label.endsWith(":"),
      ),
    ).toBe(true);
  });

  test("ordering batches preserve compiler order, duplicates, unresolved candidates, and theme revisions", async () => {
    const { NativeDesignSystemLoader } = await import(specifier);
    /**
     * Loads one stylesheet.
     *
     * @param definitions - Stylesheet text.
     * @returns Editor queries over the completed design system.
     */
    const load = (definitions: string) => {
      const loader = new NativeDesignSystemLoader();
      loader.provide(0, "theme.css", definitions, "file:///theme.css");
      return loader.editorQuery();
    };
    const first = load(
      "@utility a { color:red; } @utility b { color:blue; } @variant hover { &:hover { @slot; } }",
    );
    const values = ["b", "hover:a", "a", "a", "unknown", "unknown:a"];
    const ranks = first.candidateRanks(values);
    expect(ranks).toEqual([1, 2, 0, 0, null, null]);
    expect(first.candidateRanks([])).toEqual([]);
    for (let a = 0; a < values.length; a++) {
      for (let b = 0; b < values.length; b++) {
        const expected = first.compareCandidates(values[a], values[b]);
        expect(ranks[a] == null || ranks[b] == null ? null : Math.sign(ranks[a]! - ranks[b]!)).toBe(
          expected,
        );
      }
    }
    const second = load("@utility b { color:blue; } @utility a { color:red; }");
    expect(second.candidateRanks(["a", "b"])).toEqual([1, 0]);
    expect(first.candidateRanks(["a", "b"])).toEqual([0, 1]);
    const malformed = load(
      "@utility bad-* { @stillwind-require-properties invalid; color:--value([color]); }",
    );
    expect(() => malformed.candidateRanks(["bad-[red]"])).toThrow(/bad-\*/);
  });
}
