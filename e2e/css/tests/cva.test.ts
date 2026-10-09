import { compileSource } from "@stillwind/native";
import { afterEach, expect, test } from "vite-plus/test";

const theme = `
@utility block { display:block; }
@utility tinted { color:var(--tone); }
@utility bold { font-weight:700; }
@stillwind-slot utilities;
`;

const containers: HTMLElement[] = [];
afterEach(() => {
  for (const container of containers.splice(0)) {
    container.remove();
  }
});

for (const framework of ["react", "svelte"] as const) {
  test(`CVA applies selected CSS and ordered inline properties for ${framework}`, async () => {
    const output = compileSource({
      source: `import {style} from 'stillwind/${framework}';
let tone = 'blue';
export const choose = style.cva(['block tinted', {'--tone':tone, '--space':4}], {
  variants: {
    kind: { plain:null, fixed:['bold', {'--tone':'red'}] },
  },
  defaultVariants: {kind:'plain'},
  compoundVariants: [{kind:'fixed', class:{'--tone':'green'}}],
});
tone = 'purple';`,
      moduleId: `cva-${framework}.js`,
      uri: `untitled:cva-${framework}.js`,
      language: "js",
      moduleKind: "unambiguous",
      css: {
        content: theme,
        name: "root.css",
        uri: "untitled:root.css",
      },
    });
    expect(output.diagnostics).toEqual([]);
    expect(output.code).toBeTruthy();
    const runtime = new URL(`../../../packages/core/src/runtime/${framework}.ts`, import.meta.url)
      .href;
    const code = output.code!.replaceAll(
      `"stillwind/runtime/${framework}"`,
      JSON.stringify(runtime),
    );
    const url = URL.createObjectURL(new Blob([code], { type: "text/javascript" }));
    let choose: (props?: { kind?: string | null }) => {
      class?: string;
      className?: string;
      style?: string | Record<string, string>;
    };
    try {
      ({ choose } = await import(/* @vite-ignore */ url));
    } finally {
      URL.revokeObjectURL(url);
    }
    const container = document.createElement("section");
    containers.push(container);
    const css = document.createElement("style");
    css.textContent = output.css;
    container.append(css);
    document.body.append(container);
    for (const [kind, color, weight] of [
      [undefined, "rgb(0, 0, 255)", "400"],
      [null, "rgb(0, 0, 255)", "400"],
      ["fixed", "rgb(0, 128, 0)", "700"],
    ] as const) {
      const props = choose({ kind });
      const element = document.createElement("div");
      element.className = props.className ?? props.class ?? "";
      if (typeof props.style === "string") {
        element.style.cssText = props.style;
      } else {
        for (const [name, value] of Object.entries(props.style ?? {})) {
          element.style.setProperty(name, value);
        }
      }
      container.append(element);
      await expect.poll(() => getComputedStyle(element).color).toBe(color);
      expect(getComputedStyle(element).display).toBe("block");
      expect(getComputedStyle(element).fontWeight).toBe(weight);
      expect(element.style.getPropertyValue("--space")).toBe("4");
    }
  });
}
