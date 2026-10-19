import { readFile } from "node:fs/promises";

import { expect, test } from "vite-plus/test";

/**
 * Registers editor and canonical lint contracts for the selected native or WASM loader.
 * @param specifier - Module specifier of the binding that the tests import.
 */
export function registerEditorContract(specifier: string): void {
  test("functional registrations preserve modifiers, nested targets, theme defaults, and CSS functions", async () => {
    const { NativeDesignSystemLoader, NativeProjectSession, decodeArbitraryValue } = await import(
      specifier
    );
    const loader = new NativeDesignSystemLoader();
    loader.provide(
      0,
      "theme.css",
      `
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
    expect(query.completeCandidate("hover:", "utility", true)).toContain("hover:nth-[]:");
    expect(query.completeCandidate("hover:")).not.toContain("hover:hover:");
    expect(query.completeCandidate("nth-")).not.toContain("nth-[]:");
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
      new URL("../../core/css/builtin-variants/tw-state.css", import.meta.url),
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

  test("editor input roles share compiler registries, declaration mapping, and variant restrictions", async () => {
    const { NativeDesignSystemLoader } = await import(specifier);
    const motion = await readFile(
      new URL("../../core/css/builtin-utilities/tw-motion.css", import.meta.url),
      "utf8",
    );
    const loader = new NativeDesignSystemLoader();
    loader.provide(
      0,
      "theme.css",
      `${motion}
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
    expect(query.previewCandidate("duration-180", "keyframesFrame")).toContain(
      "transition-duration:",
    );
    expect(query.previewCandidate("duration-normal", "animationOptions")).toBe(
      "&{animation-duration:var(--duration-normal);}",
    );
    expect(query.previewCandidate("[transition-duration:75ms]", "animationOptions")).toBe(
      "&{animation-duration:75ms;}",
    );
    expect(query.previewCandidate("[transition-timing-function:linear]", "animationOptions")).toBe(
      "&{animation-timing-function:linear;}",
    );
    expect(query.previewCandidate("[transition-delay:75ms]", "animationOptions")).toBeNull();
    expect(query.completeCandidate("duration-no", "animationOptions")).toEqual(["duration-normal"]);
    expect(query.completeCandidate("ease-(--ease-so", "animationOptions")).toContain(
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
        query.completeCandidate("", context).some((label: string) => label.endsWith(":")),
      ).toBe(false);
    }
    for (const context of [
      "viewTransitionOptions",
      "viewTransitionUtilities",
      "definitionVariants",
    ]) {
      const labels = query.completeCandidate("", context);
      expect(labels).toEqual(expect.arrayContaining(["media:", "supports:", "sequence:"]));
      for (const label of ["hover:", "container:", "mixed:"]) {
        expect(labels).not.toContain(label);
      }
    }
    expect(query.previewCandidate("media:both", "viewTransitionOptions")).toContain(
      "@media print{&{animation-fill-mode:both;}}",
    );
    expect(query.previewCandidate("sequence:block", "viewTransitionUtilities")).toContain(
      "@supports (display:grid)",
    );
    expect(query.previewCandidate("hover:block", "viewTransitionUtilities")).toBeNull();
    expect(
      query
        .completeCandidate("", "definitionVariants")
        .every((label: string) => label.endsWith(":")),
    ).toBe(true);
  });

  test("module previews keep source boundaries, unused definitions, recovered calls, and captured revisions", async () => {
    const { NativeDesignSystemLoader, NativeProjectSession } = await import(specifier);
    const loader = new NativeDesignSystemLoader();
    loader.provide(
      0,
      "theme.css",
      "@utility block {display:block;} @utility flex {display:flex;} @stillwind-slot utilities; @stillwind-slot keyframes;",
    );
    const session = new NativeProjectSession(loader, true);
    /**
     * Uses one stable module identity with fresh original text and no checker facts.
     * @param code - Module text.
     * @returns A prepared source for `entry.tsx`.
     */
    const prepare = (code: string): import("../binding.js").NativePreparedSource => ({
      kind: "direct",
      moduleId: "entry.tsx",
      document: {
        uri: "file:///entry.tsx",
        code,
        format: "jsx",
      },
      syntax: {
        language: "tsx",
        moduleKind: "module",
      },
      typeFacts: [],
    });
    const source =
      'import {style} from "stillwind/react"; const 日本語="🦀"; const frames=style.keyframes({from:"block",to:"flex"}); const value=style("block");';
    session.compile([prepare(source)]);
    const captured = session.editorModules(["entry.tsx"])[0];
    expect(captured).not.toBeNull();
    expect(captured.moduleId()).toBe("entry.tsx");
    expect(session.editorModules(["unknown.tsx"])).toEqual([]);
    const input = captured
      .sites()
      .find((site: import("../binding.js").NativeEditorSite) => site.context === "keyframesFrame");
    const text = new TextDecoder().decode(
      new TextEncoder().encode(source).subarray(input.start, input.end),
    );
    expect(text).toBe("block");
    const css = captured.previewInput(input.start, "flex");
    expect(css).toContain("from{display:flex;}");
    expect(css).toContain("@keyframes __keyframes_frames");
    expect(
      captured
        .sites()
        .find((site: import("../binding.js").NativeEditorSite) => site.kind === "definition").css,
    ).toContain("to{display:flex;}");
    const revision = captured.revision();
    session.compile([prepare(source)]);
    expect(session.editorModules(["entry.tsx"])[0].revision()).toBe(revision);
    session.compile([prepare(source.replace('style("block")', 'style({"missing:":"block"})'))]);
    const recovered = session.editorModules(["entry.tsx"])[0];
    expect(
      recovered
        .sites()
        .find((site: import("../binding.js").NativeEditorSite) => site.kind === "style").valid,
    ).toBe(true);
    expect(recovered.revision()).not.toBe(revision);
    expect(
      recovered
        .sites()
        .find((site: import("../binding.js").NativeEditorSite) => site.kind === "style").css,
    ).toBe("");
    expect(captured.previewInput(input.start, "flex")).toBe(css);
    expect(captured.previewInput(input.start, "hover:flex")).toBeNull();
  });

  test("editor names stay independent of host names while preserving naming failures", async () => {
    const { NativeDesignSystemLoader, NativeProjectSession } = await import(specifier);
    const loader = new NativeDesignSystemLoader();
    loader.provide(
      0,
      "theme.css",
      "@utility block {display:block;} @stillwind-slot utilities; @stillwind-slot keyframes;",
    );
    const naming = {
      keyframesName: () => "app-animation",
    };
    const source: import("../binding.js").NativePreparedSource = {
      kind: "direct",
      moduleId: "entry.tsx",
      document: {
        uri: "file:///entry.tsx",
        code: 'import {style} from "stillwind/react"; const frames=style.keyframes({from:"block"}); const value=style(frames(""));',
        format: "jsx",
      },
      syntax: {
        language: "tsx",
        moduleKind: "module",
      },
      typeFacts: [],
    };
    const session = new NativeProjectSession(loader, true, "release", naming);
    const compiled = session.compile([source]);
    expect(compiled.modules[0].css).toContain("app-animation");
    const capture = session.editorModules(["entry.tsx", "entry.tsx"]);
    expect(capture).toHaveLength(1);
    const css = capture[0]
      .sites()
      .find((site: import("../binding.js").NativeEditorSite) => site.kind === "style").css;
    expect(css).toContain("animation-name:__keyframes_frames");
    const rejected = new NativeProjectSession(loader, true, "release", {
      keyframesName: () => {
        throw new Error("name unavailable");
      },
    });
    rejected.compile([source]);
    expect(
      rejected
        .editorModules(["entry.tsx"])[0]
        .sites()
        .filter(
          (site: import("../binding.js").NativeEditorSite) =>
            site.kind === "definition" || site.kind === "style",
        )
        .every((site: import("../binding.js").NativeEditorSite) => !site.valid && !site.css),
    ).toBe(true);
  });

  test("conditional helper names remain stable across replacement preview requests", async () => {
    const { NativeDesignSystemLoader, NativeProjectSession } = await import(specifier);
    const loader = new NativeDesignSystemLoader({
      useCssIf: false,
    });
    loader.provide(
      0,
      "theme.css",
      "@utility red {color:--if(--visible,red,blue);} @utility green {color:--if(--active,green,blue);} @stillwind-animation-option fade {animation-duration:--if(--visible,1s,2s);} @utility picture {background-image:url('asset.svg');} @stillwind-slot utilities;",
    );
    const session = new NativeProjectSession(loader, true);
    session.compile([
      {
        kind: "direct",
        moduleId: "entry.tsx",
        document: {
          uri: "file:///entry.tsx",
          code: 'import {style} from "stillwind/react"; const value=style("red");',
          format: "jsx",
        },
        syntax: {
          language: "tsx",
          moduleKind: "module",
        },
        typeFacts: [],
      },
    ]);
    const captured = session.editorModules(["entry.tsx"])[0];
    const input = captured
      .sites()
      .find((site: import("../binding.js").NativeEditorSite) => site.kind === "input");
    expect(input.css).toContain("--__if_fallback_1:var(--visible) red");
    const first = captured.previewInput(input.start, "green");
    expect(first).toContain("--__if_fallback_2:var(--active) green");
    captured.previewInput(input.start, "[color:--if(--other,black,white)]");
    expect(captured.previewInput(input.start, "green")).toBe(first);
    expect(captured.previewInput(input.start, "red")).toBe(input.css);
    const query = loader.editorQuery();
    for (const context of ["utility", "keyframesFrame", "viewTransitionUtilities"]) {
      expect(query.previewCandidate("red", context)).toContain("--__if_fallback_1");
      expect(query.previewCandidate("picture", context)).toContain("asset.svg");
    }
    for (const context of ["animationOptions", "viewTransitionOptions"]) {
      expect(query.previewCandidate("fade", context)).toContain("--__if_fallback_1");
    }
    expect(query.previewCandidate("[color:--if(--flag,red)]")).toBeNull();
  });

  test("canonical replacements obey token and unit restrictions", async () => {
    const { NativeDesignSystemLoader } = await import(specifier);
    const css =
      "@theme {--color-red-500:red;--spacing:0.25rem;} @utility bg-* {background-color:--value(--color-*,[color]);} @utility p-* {padding:--value(--spacing(),[length]);} @stillwind-slot utilities;";
    /**
     * Loads the shared stylesheet with compiler options.
     * @param options - Options under test.
     * @returns Editor queries over the completed design system.
     */
    const load = (options: import("../binding.js").NativeCompilerOptions) => {
      const loader = new NativeDesignSystemLoader(options);
      loader.provide(0, "theme.css", css, "file:///theme.css");
      return loader.editorQuery();
    };
    expect(load({}).canonicalCandidate("bg-[rgb(255_0_0)]")).toBe("bg-red-500");
    expect(
      load({
        lint: {
          noRestrictedThemeTokens: {
            tokens: ["--color-red-*"],
          },
        },
      }).canonicalCandidate("bg-[rgb(255_0_0)]"),
    ).toBeNull();
    expect(load({}).canonicalCandidate("p-[16px]")).toBe("p-4");
    expect(
      load({
        lint: {
          noRestrictedThemeTokens: {
            tokens: ["--spacing"],
          },
        },
      }).canonicalCandidate("p-[16px]"),
    ).toBeNull();
    expect(
      load({
        lint: {
          noRestrictedLengthUnits: {
            rules: [
              {
                properties: ["padding*"],
                disallow: ["rem"],
              },
            ],
          },
        },
      }).canonicalCandidate("p-[16px]"),
    ).toBeNull();
  });

  test("a loader that defers its canonical index answers canonical queries the same way", async () => {
    const { NativeDesignSystemLoader } = await import(specifier);
    const loader = new NativeDesignSystemLoader();
    loader.deferCanonicalIndex();
    expect(() => loader.prepareCanonicalIndex()).toThrow();
    loader.provide(
      0,
      "theme.css",
      "@theme {--spacing:0.25rem;} @utility p-* {padding:--value(--spacing(),[length]);} @stillwind-slot utilities;",
      "file:///theme.css",
    );
    expect(loader.editorQuery().canonicalCandidate("p-[16px]")).toBe("p-4");
    loader.prepareCanonicalIndex();
    expect(loader.editorQuery().canonicalCandidate("p-[16px]")).toBe("p-4");
  });

  test("ordering batches preserve compiler order, duplicates, unresolved candidates, and theme revisions", async () => {
    const { NativeDesignSystemLoader } = await import(specifier);
    /**
     * Loads one stylesheet.
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

  test("canonical queries preserve aliases, required registrations, and modifier branches", async () => {
    const { NativeDesignSystemLoader } = await import(specifier);
    const loader = new NativeDesignSystemLoader();
    loader.provide(
      0,
      "theme.css",
      `@theme { --*:initial; --spacing-gap:1rem; --color-a:red; --color-b:blue; --color-special-a:blue; --color-alias:var(--color-a); --text-body:1rem; --leading-tight:1.25rem; }
@property --flag { syntax:'*'; inherits:false; initial-value:0; }
@utility tone-* { @stillwind-require-properties --flag; &:hover { color:--value(--color-*, [color]); outline-color:var(--color-alias); content:'var(--color-a)'; } }
@utility text-* { font-size:--value(--text-*, [length]); line-height:--modifier(--leading-*, [length]); }
@utility -m-* { margin:calc(--value(--spacing-*, [length]) * -1); }
@utility red { @stillwind-require-properties --flag; color:red; }
@utility overlap-* { color:--value(--color-*, [color]); opacity:--modifier(number); }
@utility overlap-special-* { color:--value(--color-*, [color]); opacity:--modifier(number); }
@utility overlap-a { color:blue; opacity:--modifier(number); }
@utility overlap-alias { color:blue; opacity:--modifier(number); }
@stillwind-slot utilities;`,
      "file:///theme.css",
    );
    const query = loader.editorQuery();
    expect(query.canonicalCandidate("tone-[#f00]")).toBe("tone-a");
    expect(query.canonicalCandidate("tone-[#00f]")).toBe("tone-b");
    expect(query.canonicalCandidate("text-[1rem]/tight")).toBe("text-body/tight");
    expect(query.canonicalCandidate("[margin:-1rem]")).toBe("-m-gap");
    expect(query.canonicalCandidate("[color:red]")).toBeNull();
    expect(query.canonicalCandidate("overlap-[rgb(255_0_0)]/50")).toBe("overlap-special-a/50");
  });

  test("CSS loading prepares canonical indexes without reporting unused requirement errors", async () => {
    const { NativeDesignSystemLoader } = await import(specifier);
    const loader = new NativeDesignSystemLoader();
    const loaded = loader.provide(
      0,
      "theme.css",
      "@utility good { color:red; } @utility bad-* { @stillwind-require-properties invalid; color:--value([color]); }",
      "file:///theme.css",
    );
    expect(loaded.kind).toBe("complete");
    const query = loader.editorQuery();
    expect(query.previewCandidate("good")).not.toBeNull();
    expect(() => query.canonicalCandidate("bad-[red]")).toThrow(/bad-\*/);
  });

  test("canonical indexes preserve old snapshots and isolate theme reloads", async () => {
    const { NativeDesignSystemLoader } = await import(specifier);
    const loader = new NativeDesignSystemLoader();
    /**
     * Builds a theme whose brand color differs between revisions.
     * @param value - Brand color.
     * @returns Stylesheet text.
     */
    const css = (value: string) =>
      `@theme { --color-*:initial; --color-brand:${value}; }
@utility bg-* { background-color:--value(--color-*, [color]); }
@variant over { &:hover { @slot; } }
@stillwind-slot utilities;`;
    loader.provide(0, "theme.css", css("red"), "file:///theme.css");
    const original = loader.editorQuery();
    expect(original.canonicalCandidate("bg-[#f00]")).toBe("bg-brand");
    expect(original.canonicalCandidate("[&:hover]:bg-brand")).toBe("over:bg-brand");
    expect(loader.editorQuery().canonicalCandidate("bg-[#f00]")).toBe("bg-brand");
    const reloaded = new NativeDesignSystemLoader();
    reloaded.provide(0, "theme.css", css("blue"), "file:///theme.css");
    const updated = reloaded.editorQuery();
    expect(updated.canonicalCandidate("bg-[#f00]")).toBeNull();
    expect(updated.canonicalCandidate("bg-[#00f]")).toBe("bg-brand");
    expect(original.canonicalCandidate("bg-[#f00]")).toBe("bg-brand");
    expect(original.canonicalCandidate("bg-[#00f]")).toBeNull();
  });

  test("editor queries and compiler diagnostics share canonical options and source suppression", async () => {
    const { NativeDesignSystemLoader, NativeProjectSession, fingerprint } = await import(specifier);
    const loader = new NativeDesignSystemLoader({
      lint: {
        preferCanonicalUtilities: {
          severity: "deny",
        },
      },
      rootFontSize: 20.5,
    });
    loader.provide(
      0,
      "theme.css",
      `@theme { --spacing:.25rem; --color-brand:red; --text-body:1rem; --leading-tight:1.25rem; --breakpoint-narrow:40rem; }
@utility p-* { padding:--value(--spacing-*, --spacing(), [length]); }
@utility bg-* { background-color:--value(--color-*, [color]); }
@utility text-* { font-size:--value(--text-*, [length]); line-height:--modifier(--leading-*, [length]); }
@variant hover { &:hover { @slot; } }
@stillwind-variant group-* (value: [variant]) { &:is(:where(.group):--value() *) { @slot; } }
@variant narrow-* { @media (width < --value(--breakpoint-*, [length])) { @slot; } }
@stillwind-slot utilities;`,
      "file:///theme.css",
    );
    const query = loader.editorQuery();
    expect(query.canonicalOptions()).toMatchObject({
      severity: "deny",
      pxToRem: true,
    });
    expect(query.canonicalCandidate("p-[20.5px]")).toBe("p-4");
    expect(query.previewCandidate("hover:bg-brand")).toContain("&:hover");
    expect(query.formatCss("&{color:red;&:hover{color:blue}}")).toBe(
      "& {\n  color: red;\n\n  &:hover {\n    color: #00f;\n  }\n}\n",
    );
    expect(() => query.formatCss("&{color red}")).toThrow(/CSS parse failed/);
    expect(
      query.formatCss("&{padding:calc(var(--spacing) * 4);color:var(--color-brand)}", {}),
    ).toBe(
      "& {\n  padding: calc(var(--spacing) * 4) /* 1rem = 20.5px */;\n  color: var(--color-brand) /* red = #ff0000 */;\n}\n",
    );
    expect(
      query.formatCss("&{padding:calc(var(--spacing) * 4)}", {
        themeValues: false,
        colorEquivalents: false,
      }),
    ).toBe("& {\n  padding: calc(var(--spacing) * 4);\n}\n");
    expect(() =>
      query.formatCss("&{color:red}", {
        colorEquivalents: true as unknown as false,
      }),
    ).toThrow("colorEquivalents");
    expect(query.completeCandidate("bg-br")).toContain("bg-brand");
    expect(query.completeCandidate("bg-(--color-br")).toContain("bg-(--color-brand)");
    expect(query.colorRgba("oklch(60% 0 0)")).toHaveLength(4);
    expect(query.colorRgba("currentColor")).toBeNull();
    expect(query.candidateColor("hover:bg-brand")).toEqual([1, 0, 0, 1]);
    expect(query.candidateColor("bg-[currentColor]")).toBeNull();
    expect(query.completeCandidate("hov")).toContain("hover:");
    expect(query.completeCandidate("group-hov")).toContain("group-hover:");
    expect(query.completeCandidate("!bg-br")).toContain("!bg-brand");
    expect(query.completeCandidate("text-body/ti")).toContain("text-body/tight");
    expect(query.completeCandidate("narrow-na")).toContain("narrow-narrow:");
    expect(query.completeCandidate("bg-[var(--color-br")).toContain("bg-[var(--color-brand)]");
    const code = String.raw`import {style} from 'stillwind/react';
const 日本語='🦀'; const card=style('p-\u005b20.5px\u005d');
// swlint-disable-next-line prefer-canonical-utilities
const ignored=style('p-[20.5px]');`;
    const session = new NativeProjectSession(loader, false);
    const input = {
      kind: "direct",
      moduleId: "entry.tsx",
      document: {
        uri: "file:///entry.tsx",
        code,
        format: "jsx",
      },
      syntax: {
        language: "tsx",
        moduleKind: "module",
      },
      typeFacts: [],
    };
    const reports = session
      .diagnose([input])[0]
      .reports.filter((report: { code: string }) => report.code === "prefer-canonical-utilities");
    expect(reports).toHaveLength(1);
    expect(reports[0]).toMatchObject({
      severity: "error",
      primary: {
        location: {
          revision: fingerprint(code),
          startUtf16: code.indexOf(String.raw`p-\u005b`),
        },
      },
    });
    expect(reports[0].actions[0].edits[0].replacement).toBe("p-4");
    const compiled = session
      .compile([input])
      .modules[0].reports.filter(
        (report: { code: string }) => report.code === "prefer-canonical-utilities",
      );
    expect(compiled).toEqual(reports);
  });

  test("stylesheet reports use the lint settings of the loader and its session", async () => {
    const { NativeDesignSystemLoader, NativeProjectSession } = await import(specifier);
    const css = `@custom-variant wide (@media (width >= 40rem));
.card { @media (min-width: 640px) { color: red; } }
`;
    /**
     * Loads `css` as a linted source with `options`.
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
