import { expect, test } from "vite-plus/test";

import { installSandboxGuards, MemoryStorage } from "./sandbox.ts";

test("memory storage implements the Storage methods", () => {
  const storage = new MemoryStorage();
  storage.setItem("theme", "dark");
  storage.setItem("count", 1 as unknown as string);
  expect(storage.getItem("theme")).toBe("dark");
  expect(storage.getItem("count")).toBe("1");
  expect(storage.getItem("missing")).toBeNull();
  expect([storage.length, storage.key(0), storage.key(2)]).toEqual([2, "theme", null]);
  storage.removeItem("theme");
  expect(storage.length).toBe(1);
  storage.clear();
  expect(storage.length).toBe(0);
});

test("the guards cancel form submission and replace denied storage", () => {
  class Frame extends EventTarget {
    get localStorage(): Storage {
      throw new DOMException("Access is denied", "SecurityError");
    }
    readonly sessionStorage = new MemoryStorage();
  }
  const frame = new Frame();
  const sessionStorage = frame.sessionStorage;
  installSandboxGuards(frame as unknown as Window);

  const submit = new Event("submit", { cancelable: true });
  let handled = false;
  frame.addEventListener("submit", () => (handled = true));
  frame.dispatchEvent(submit);
  expect([submit.defaultPrevented, handled]).toEqual([true, true]);

  expect(frame.localStorage).toBeInstanceOf(MemoryStorage);
  expect(frame.sessionStorage).toBe(sessionStorage);
});
