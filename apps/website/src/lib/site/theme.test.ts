import { expect, it } from "vite-plus/test";

import { mountThemeControl } from "./theme";

function themeControl(preference?: string) {
  const root = { dataset: { theme: preference } };
  const attributes = new Map<string, string>();
  const stored = new Map<string, string>();
  const status = { textContent: "" };
  const view = Object.assign(new EventTarget(), {
    localStorage: {
      setItem: (key: string, value: string) => stored.set(key, value),
      removeItem: (key: string) => stored.delete(key),
    },
  });
  const button = Object.assign(new EventTarget(), {
    ownerDocument: { documentElement: root, defaultView: view },
    hidden: true,
    title: "",
    setAttribute: (key: string, value: string) => attributes.set(key, value),
    querySelector: () => status,
  });
  const dispose = mountThemeControl(button as unknown as HTMLButtonElement);
  return {
    root,
    attributes,
    stored,
    status,
    button,
    dispose,
    click: () => button.dispatchEvent(new Event("click")),
    synchronize(key: string | null, newValue: string | null) {
      view.dispatchEvent(Object.assign(new Event("storage"), { key, newValue }));
    },
    blockStorage() {
      Object.defineProperty(view, "localStorage", {
        get() {
          throw new Error("Storage is disabled");
        },
      });
    },
  };
}

it("cycles the accessible control and saves explicit choices while system uses the OS", () => {
  const control = themeControl();
  expect(control.button.hidden).toBe(false);
  expect(control.attributes.get("aria-label")).toBe("Color theme: system. Switch to light.");
  control.click();
  expect(control.root.dataset.theme).toBe("light");
  expect(control.stored.get("stillwind:theme")).toBe("light");
  control.click();
  expect(control.root.dataset.theme).toBe("dark");
  expect(control.stored.get("stillwind:theme")).toBe("dark");
  expect(control.status.textContent).toBe("Color theme: dark");
  control.click();
  expect(control.root.dataset.theme).toBe("system");
  expect(control.stored.has("stillwind:theme")).toBe(false);
  control.dispose();
});

it("synchronizes other tabs and releases click and storage listeners on disposal", () => {
  const control = themeControl("dark");
  control.synchronize("unrelated", "light");
  expect(control.root.dataset.theme).toBe("dark");
  control.synchronize("stillwind:theme", "light");
  expect(control.root.dataset.theme).toBe("light");
  expect(control.button.title).toBe("Color theme: light. Switch to dark.");
  control.synchronize(null, null);
  expect(control.root.dataset.theme).toBe("system");
  control.dispose();
  control.click();
  control.synchronize("stillwind:theme", "dark");
  expect(control.root.dataset.theme).toBe("system");
});

it("keeps theme switching available when preference storage is disabled", () => {
  const control = themeControl();
  control.blockStorage();
  expect(() => control.click()).not.toThrow();
  expect(control.root.dataset.theme).toBe("light");
  expect(() => control.click()).not.toThrow();
  expect(control.root.dataset.theme).toBe("dark");
  control.dispose();
});
