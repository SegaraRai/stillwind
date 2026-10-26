import { readFile } from "node:fs/promises";

import { expect, test } from "vite-plus/test";

/**
 * Registers stylesheet editing tests for the selected native or WASM binding.
 *
 * @param specifier - Absolute URL of the binding module.
 */
export function registerStylesheetEditorContract(specifier: string): void {
  test("CSS directive discovery separates registry settings, local syntax, and hover", async () => {
    const { NativeDesignSystemLoader } = await import(specifier);
    const loader = new NativeDesignSystemLoader();
    loader.provide(0, "theme.css", "");
    const query = loader.editorQuery();
    const source = "/* 日本語 🦀 */ @stillwind-";
    const offset = new TextEncoder().encode(source).length;
    const ordinary = query.completeStylesheet(source, offset);
    expect(ordinary.map((item: { label: string }) => item.label)).toContain("@stillwind-slot");
    expect(ordinary.map((item: { label: string }) => item.label)).not.toContain(
      "@stillwind-utility",
    );
    const registry = query.completeStylesheet(source, offset, { registryDirectives: true });
    expect(registry).toContainEqual(
      expect.objectContaining({
        label: "@stillwind-utility",
        detail: "Registry authoring · Stillwind syntax",
        start: new TextEncoder().encode("/* 日本語 🦀 */ ").length,
        end: offset,
      }),
    );
    expect(registry.map((item: { label: string }) => item.label)).not.toContain(
      "@stillwind-intrinsic",
    );
    const local = "@stillwind-variant named (value: [variant]) { @";
    expect(
      query.completeStylesheet(local, local.length).map((item: { label: string }) => item.label),
    ).toContain("@stillwind-intrinsic");
    const intrinsic = "@stillwind-intrinsic builtin-";
    expect(
      query
        .completeStylesheet(intrinsic, intrinsic.length)
        .map((item: { label: string }) => item.label),
    ).toEqual(["builtin-if-variant", "builtin-not-variant"]);
    const written = "@stillwind-utility";
    expect(query.hoverStylesheet(written, written.length).documentation).toContain(
      "Registry authoring",
    );
  });

  test("CSS color ranges and sorting policies share native and WASM contracts", async () => {
    const { NativeDesignSystemLoader } = await import(specifier);
    const loader = new NativeDesignSystemLoader({
      stylesheetSort: {
        applyStatements: true,
        variantBlocks: true,
      },
    });
    loader.provide(
      0,
      "theme.css",
      "@theme { --color-brand:red; } @utility a {display:block;} @utility b {padding:1px;} @utility bg-* {background-color:--value(--color-*);} @custom-variant hover (&:hover);",
    );
    const query = loader.editorQuery();
    const source = "/* 日本語 🦀 */ .a { @apply hover:bg-brand; color:var(--color-brand); }";
    const bytes = new TextEncoder().encode(source);
    const colors = query.stylesheetColors(source);
    expect(
      colors.map((color: { start: number; end: number }) =>
        new TextDecoder().decode(bytes.slice(color.start, color.end)),
      ),
    ).toEqual(["hover:bg-brand", "var(--color-brand)"]);
    expect(colors[0]).toMatchObject({
      red: 1,
      green: 0,
      blue: 0,
      alpha: 1,
      candidate: "hover:bg-brand",
    });
    const css = ".a { @apply b; @apply a; }";
    expect(query.sortStylesheet(css)).toEqual([
      {
        start: 5,
        end: css.length - 2,
        replacement: "@apply a; @apply b;",
      },
    ]);
    expect(
      query.sortStylesheet(css, [
        {
          start: 0,
          end: css.length,
        },
      ]),
    ).toEqual([]);
    expect(() =>
      query.sortStylesheet(css, [
        {
          start: 100,
          end: 101,
        },
      ]),
    ).toThrow("Invalid protected CSS range");
  });

  test("stylesheet queries preserve byte ranges and resolve incomplete applications, variants, and theme references", async () => {
    const { NativeDesignSystemLoader, editorCssSource } = await import(specifier);
    const loader = new NativeDesignSystemLoader();
    loader.provide(
      0,
      "theme.css",
      "@theme { --color-brand:red; } @utility flex { display:flex; } @custom-variant hover (&:hover);",
    );
    const query = loader.editorQuery();
    const source = "/* 日本語 🦀 */ .a { @apply fl";
    const offset = new TextEncoder().encode(source).length;
    expect(query.completeStylesheet(source, offset)).toEqual([
      { label: "flex", start: offset - 2, end: offset, keyword: false, variant: false },
    ]);
    expect(query.hoverStylesheet(".a { @variant hover", 19)?.css).toContain("&:hover");
    const reference = ".a { color:--theme(--color-br";
    expect(
      query
        .completeStylesheet(reference, reference.length)
        .map((item: { label: string }) => item.label),
    ).toEqual(["--color-brand"]);
    expect(query.revision()).toBeTruthy();
    const recovered = new NativeDesignSystemLoader();
    recovered.provide(
      0,
      "incomplete.css",
      editorCssSource(
        "@theme { --color-local:blue; } @utility local { color:var(--color-local); } .a { @apply lo",
      ),
    );
    expect(
      recovered
        .editorQuery()
        .completeStylesheet(".a { @apply lo", 14)
        .map((item: { label: string }) => item.label),
    ).toContain("local");
  });

  test("recovered CSS keeps built-in variant completions in incomplete buffers", async () => {
    const { NativeDesignSystemLoader, editorCssSource } = await import(specifier);
    const definitions = await readFile(
      new URL("../../../core/css/builtin-variants/tw-state.css", import.meta.url),
      "utf8",
    );
    for (const tail of [".a { @variant ho", ".a { @variant ho {} } .b {"]) {
      const source = `${definitions}\n${tail}`;
      const offset = new TextEncoder().encode(
        source.slice(0, source.lastIndexOf("@variant ho") + "@variant ho".length),
      ).length;
      const loader = new NativeDesignSystemLoader();
      loader.provide(0, "incomplete.css", editorCssSource(source));
      expect(
        loader
          .editorQuery()
          .completeStylesheet(source, offset)
          .map((item: { label: string }) => item.label),
      ).toEqual(["hover"]);
    }
  });

  test("stylesheet reports use the lint settings of the loader and its session", async () => {
    const { NativeDesignSystemLoader, NativeProjectSession } = await import(specifier);
    const css = `@custom-variant wide (@media (width >= 40rem));
.card { @media (min-width: 640px) { color: red; } }
`;
    /**
     * Loads `css` as a linted source with `options`.
     *
     * @param options - Loader options, whose lint settings stylesheet reports use.
     * @returns The completed loader.
     */
    const load = (options?: object) => {
      const loader = new NativeDesignSystemLoader(options);
      loader.provide(0, "root.css", css, "file:///project/root.css", null, true);
      return loader;
    };
    /**
     * Selects the reports of the rule under test.
     *
     * @param reports - Stylesheet reports.
     * @returns The `prefer-variant-directive` reports.
     */
    const directives = (reports: { code: string }[]) =>
      reports.filter((report) => report.code === "prefer-variant-directive");
    expect(directives(load().stylesheetReports())).toEqual([]);
    const loader = load({
      lint: {
        preferVariantDirective: {
          severity: "warn",
          pxToRem: true,
        },
      },
    });
    const reports = directives(loader.stylesheetReports());
    expect(reports).toHaveLength(1);
    expect(reports[0]).toMatchObject({
      severity: "warning",
      actions: [
        {
          edits: [
            {
              replacement: "@variant wide",
            },
          ],
        },
      ],
    });
    expect(directives(new NativeProjectSession(loader, false).stylesheetReports())).toEqual(
      reports,
    );
  });
}
