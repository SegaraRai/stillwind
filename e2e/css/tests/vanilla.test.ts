import { compileSource } from "@stillwind/native";
import { applyStyle, clearStyle, type StyleProps } from "stillwind/vanilla";
import { afterEach, expect, test } from "vite-plus/test";

let container: HTMLElement | undefined;
afterEach(() => {
  container?.remove();
  container = undefined;
});

test("compiled vanilla props update computed CSS and restore existing inline values", async () => {
  const output = compileSource({
    source: `import { style } from "stillwind/vanilla";
export function props(active, tone) {
  return style(active ? "expanded" : "compact", {"--tone": tone}, active && {"--size": 4});
}`,
    moduleId: "vanilla.js",
    uri: "untitled:vanilla.js",
    language: "js",
    moduleKind: "module",
    css: {
      content:
        "@utility expanded {padding:20px;color:var(--tone);}@utility compact {padding:8px;color:var(--tone);}@stillwind-slot utilities;",
      name: "root.css",
      uri: "untitled:root.css",
    },
  });
  expect(output.diagnostics).toEqual([]);
  expect(output.code).toBeTruthy();
  const runtime = new URL("../../../packages/core/src/runtime/class-object.ts", import.meta.url)
    .href;
  const code = output.code!.replaceAll('"stillwind/runtime/class-object"', JSON.stringify(runtime));
  const url = URL.createObjectURL(new Blob([code], { type: "text/javascript" }));
  let props: (active: boolean, tone: string) => StyleProps;
  try {
    ({ props } = await import(/* @vite-ignore */ url));
  } finally {
    URL.revokeObjectURL(url);
  }

  container = document.createElement("section");
  const css = document.createElement("style");
  css.textContent = output.css;
  const button = document.createElement("button");
  button.className = "existing";
  button.style.setProperty("--tone", "green", "important");
  container.append(css, button);
  document.body.append(container);

  applyStyle(button, props(true, "red"));
  await expect.poll(() => getComputedStyle(button).color).toBe("rgb(255, 0, 0)");
  expect(getComputedStyle(button).paddingTop).toBe("20px");
  expect(button.style.getPropertyValue("--size")).toBe("4");
  const expanded = button.className;

  applyStyle(button, props(false, "blue"));
  await expect.poll(() => getComputedStyle(button).color).toBe("rgb(0, 0, 255)");
  expect(getComputedStyle(button).paddingTop).toBe("8px");
  expect(button.className).not.toBe(expanded);
  expect(button.style.getPropertyValue("--size")).toBe("");

  clearStyle(button);
  expect(button.className).toBe("existing");
  expect(button.style.getPropertyValue("--tone")).toBe("green");
  expect(button.style.getPropertyPriority("--tone")).toBe("important");
});

test("DOM ownership keeps caller mutations, external writes, SVG, and empty updates independent", () => {
  const element = document.createElementNS("http://www.w3.org/2000/svg", "svg");
  element.classList.add("shared");
  element.style.setProperty("--restore", "original", "important");
  element.style.setProperty("--untouched", "keep");
  const props = {
    class: "shared added non\u00a0breaking",
    style: { "--restore": "temporary", "--stale": "remove", "--external": "first" },
  };
  applyStyle(element, props);
  expect(element.classList.contains("non\u00a0breaking")).toBe(true);
  element.classList.add("external");
  element.style.setProperty("--external", "second");
  props.class = "replacement";
  delete (props.style as Partial<typeof props.style>)["--stale"];
  applyStyle(element, props);
  expect([...element.classList]).toEqual(["shared", "external", "replacement"]);
  expect(element.style.getPropertyValue("--stale")).toBe("");
  expect(element.style.getPropertyValue("--external")).toBe("first");

  applyStyle(element, {});
  expect([...element.classList]).toEqual(["shared", "external"]);
  expect(element.style.getPropertyValue("--restore")).toBe("original");
  expect(element.style.getPropertyPriority("--restore")).toBe("important");
  expect(element.style.getPropertyValue("--external")).toBe("second");
  expect(element.style.getPropertyValue("--untouched")).toBe("keep");

  applyStyle(element, { style: { "--external": "third" } });
  element.style.setProperty("--external", "latest");
  clearStyle(element);
  clearStyle(element);
  expect(element.style.getPropertyValue("--external")).toBe("latest");
});
