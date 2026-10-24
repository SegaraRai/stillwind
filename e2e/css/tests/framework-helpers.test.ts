import Alpine from "alpinejs";
import { html, nothing, render } from "lit";
import { stillwind, type StyleProps } from "stillwind/alpine";
import { applyStyle } from "stillwind/lit";
import { afterEach, expect, test } from "vite-plus/test";

Alpine.plugin(stillwind);
Alpine.start();

let container: HTMLElement | undefined;
afterEach(() => {
  container?.remove();
  container = undefined;
});

test("Alpine replaces reactive props and clears its contribution when the directive is removed", async () => {
  Alpine.data("styleProbe", () => ({
    props: { class: "first", style: { "--tone": "red", "--size": "4" } },
  }));
  container = document.createElement("section");
  container.innerHTML = `<div x-data="styleProbe"><svg class="existing" style="--tone: green !important" x-sw="props"></svg></div>`;
  Alpine.mutateDom(() => document.body.append(container!));
  Alpine.initTree(container);
  const scope = container.firstElementChild as HTMLElement;
  const element = scope.firstElementChild as SVGElement;
  expect([...element.classList]).toEqual(["existing", "first"]);
  expect(element.style.getPropertyValue("--tone")).toBe("red");

  const state = Alpine.$data(scope) as { props: StyleProps };
  state.props = { class: "second", style: { "--tone": "blue" } };
  await expect.poll(() => [...element.classList]).toEqual(["existing", "second"]);
  expect(element.style.getPropertyValue("--tone")).toBe("blue");
  expect(element.style.getPropertyValue("--size")).toBe("");

  element.removeAttribute("x-sw");
  await expect.poll(() => [...element.classList]).toEqual(["existing"]);
  expect(element.style.getPropertyValue("--tone")).toBe("green");
  expect(element.style.getPropertyPriority("--tone")).toBe("important");
});

test("Lit updates compiled props, clears on disconnect, and reapplies on reconnect", () => {
  container = document.createElement("section");
  document.body.append(container);
  const view = (props: Parameters<typeof applyStyle>[0]) =>
    html`<div class="existing" style="--tone: green !important" ${applyStyle(props)}></div>`;
  const part = render(
    view({ class: "first", style: { "--tone": "red", "--size": "4" } }),
    container,
  );
  const element = container.querySelector("div")!;
  expect([...element.classList]).toEqual(["existing", "first"]);
  render(view({ class: "second", style: { "--tone": "blue" } }), container);
  expect([...element.classList]).toEqual(["existing", "second"]);
  expect(element.style.getPropertyValue("--size")).toBe("");

  part.setConnected(false);
  expect([...element.classList]).toEqual(["existing"]);
  expect(element.style.getPropertyValue("--tone")).toBe("green");
  expect(element.style.getPropertyPriority("--tone")).toBe("important");
  render(view({ class: "third", style: { "--tone": "purple" } }), container);
  expect([...element.classList]).toEqual(["existing"]);
  expect(element.style.getPropertyValue("--tone")).toBe("green");
  part.setConnected(true);
  expect([...element.classList]).toEqual(["existing", "third"]);
  expect(element.style.getPropertyValue("--tone")).toBe("purple");

  render(nothing, container);
  expect([...element.classList]).toEqual(["existing"]);
  expect(element.style.getPropertyValue("--tone")).toBe("green");
});

test("Lit rejects a directive outside an element binding", () => {
  container = document.createElement("section");
  expect(() => render(html`<div>${applyStyle({ class: "invalid" })}</div>`, container!)).toThrow(
    "Stillwind applyStyle must be used in a Lit element binding",
  );
});
