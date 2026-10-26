import { expect, test } from "vite-plus/test";

/**
 * Registers CSS preview tests for the selected native or WASM binding.
 *
 * @param specifier - Absolute URL of the binding module.
 */
export function registerPreviewsEditorContract(specifier: string): void {
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
     *
     * @param code - Module text.
     * @returns A prepared source for `entry.tsx`.
     */
    const prepare = (code: string): import("../../binding.js").NativePreparedSource => ({
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
      .find(
        (site: import("../../binding.js").NativeEditorSite) => site.context === "keyframesFrame",
      );
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
        .find((site: import("../../binding.js").NativeEditorSite) => site.kind === "definition")
        .css,
    ).toContain("to{display:flex;}");
    const revision = captured.revision();
    session.compile([prepare(source)]);
    expect(session.editorModules(["entry.tsx"])[0].revision()).toBe(revision);
    session.compile([prepare(source.replace('style("block")', 'style({"missing:":"block"})'))]);
    const recovered = session.editorModules(["entry.tsx"])[0];
    expect(
      recovered
        .sites()
        .find((site: import("../../binding.js").NativeEditorSite) => site.kind === "style").valid,
    ).toBe(true);
    expect(recovered.revision()).not.toBe(revision);
    expect(
      recovered
        .sites()
        .find((site: import("../../binding.js").NativeEditorSite) => site.kind === "style").css,
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
    const source: import("../../binding.js").NativePreparedSource = {
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
      .find((site: import("../../binding.js").NativeEditorSite) => site.kind === "style").css;
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
          (site: import("../../binding.js").NativeEditorSite) =>
            site.kind === "definition" || site.kind === "style",
        )
        .every((site: import("../../binding.js").NativeEditorSite) => !site.valid && !site.css),
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
      "@utility red {color:--if(--visible,red,blue);} @utility green {color:--if(--active,green,blue);} @stillwind-registry animation, view-transition { @utility fade {animation-duration:--if(--visible,1s,2s);} } @utility picture {background-image:url('asset.svg');} @stillwind-slot utilities;",
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
      .find((site: import("../../binding.js").NativeEditorSite) => site.kind === "input");
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
    for (const context of ["animationOptions", "viewTransitionUtilities"]) {
      expect(query.previewCandidate("fade", context)).toContain("--__if_fallback_1");
    }
    expect(query.previewCandidate("[color:--if(--flag,red)]")).toBeNull();
  });
}
