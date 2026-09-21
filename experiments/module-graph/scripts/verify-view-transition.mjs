import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { runInNewContext } from "node:vm";

import { chromium } from "playwright";

import * as runtime from "../../../packages/core/src/runtime/react.ts";
import { compileSource } from "../../../packages/native/index.js";

const source = `import {style} from "stillwind/react";
const item = style.viewTransition({ group: "duration-100 ease-linear", old: "opacity-0", new: "opacity-100" });
const aside = style.viewTransition({ old: "opacity-100" });
export const staticallyKeyed = style("block", item.for("hero"));
export const dynamicallyKeyed = style("block", item.for(key));
export const unioned = style("block", item.for("one"), aside.for("two"));
export const guarded = style("block", cond && item.for("guarded"));
export const guardedTogether = style(
  "block",
  cond && item.for("guarded"),
  other && aside.for("other"),
  item.for("base"),
);
export const unnamed = style("block", item);`;
const coreRoot = resolve(fileURLToPath(new URL(".", import.meta.url)), "../../../packages/core");

/// Expands the package own imports, so the compiled theme carries the built-in registry.
function expand(css, directory = coreRoot) {
  return css.replace(/@import\s+(["'])([^"']+)\1([^;]*);/g, (rule, _quote, name, tail) => {
    const path =
      name === "stillwind"
        ? resolve(coreRoot, "index.css")
        : name.startsWith("stillwind/")
          ? resolve(coreRoot, "css", name.slice("stillwind/".length))
          : name.startsWith("./")
            ? resolve(directory, name)
            : null;
    if (!path) return rule;
    const body = expand(readFileSync(path, "utf8"), dirname(path));
    const layer = /^\s*layer\(([^)]+)\)\s*$/.exec(tail);
    if (layer) return `@layer ${layer[1]}{${body}}`;
    return tail.trim() === "" ? body : rule;
  });
}

const theme = `${expand(readFileSync(resolve(coreRoot, "index.css"), "utf8"))}
@theme { --duration-100: 100ms; --ease-linear: linear; }`;

// The compiler emits the module; the runtime key and the test come from outside it.
const output = compileSource(source, "view-transition.tsx", theme);
assert.deepEqual(output.diagnostics, []);
const sandbox = { ...runtime, key: "item 1", cond: true, other: true };
const moduleCode = output.code
  .replace(/^import .*?;$/m, "")
  .replaceAll("export const ", "globalThis.");
runInNewContext(moduleCode, sandbox);
const props = sandbox;
assert.match(props.staticallyKeyed.className, /sw_/);
assert.ok(props.dynamicallyKeyed.style["view-transition-name"]);
assert.equal(props.unnamed.style, undefined);

const browser = await chromium.launch({ channel: "chrome", headless: true });
try {
  const page = await browser.newPage();
  await page.setContent(
    `<style>${output.css}</style>` +
      [
        ["static", props.staticallyKeyed],
        ["dynamic", props.dynamicallyKeyed],
        ["unioned", props.unioned],
        ["guarded", props.guarded],
        ["guardedTogether", props.guardedTogether],
        ["unnamed", props.unnamed],
      ]
        .map(([id, value]) => `<div id="${id}" class="${value.className}"></div>`)
        .join(""),
  );
  await page.evaluate((value) => {
    const target = document.querySelector("#dynamic");
    for (const [name, entry] of Object.entries(value.style ?? {})) {
      target.style.setProperty(name, String(entry));
    }
  }, props.dynamicallyKeyed);

  const names = await page.evaluate(() => {
    const read = (id) => getComputedStyle(document.querySelector(`#${id}`)).viewTransitionName;
    return {
      static: read("static"),
      dynamic: read("dynamic"),
      unioned: read("unioned"),
      guarded: read("guarded"),
      guardedTogetherClass: getComputedStyle(
        document.querySelector("#guardedTogether"),
      ).getPropertyValue("view-transition-class"),
      unnamed: read("unnamed"),
    };
  });

  // A literal key becomes a declaration, and the name is the class token plus the key.
  assert.match(names.static, /^sw-vt-[0-9a-z]{5}-hero$/, JSON.stringify(names));
  // A runtime key is escaped for a value position, so the space keeps its escape.
  assert.equal(names.dynamic, `${names.static.replace(/-hero$/, "")}-item\\ 1`);
  // Two names in one call leave the last declaration as the element value.
  assert.match(names.unioned, /-two$/, JSON.stringify(names));
  // A guarded use marks its element while its test holds.
  assert.match(names.guarded, /-guarded$/, JSON.stringify(names));
  // Two conditions that hold together contribute both tokens to the resolved class list.
  assert.equal(names.guardedTogetherClass.split(/\s+/).length, 3, JSON.stringify(names));
  // An unkeyed use marks a class only, so it does not participate.
  assert.equal(names.unnamed, "none", JSON.stringify(names));
  assert.notEqual(names.static, names.dynamic);

  // The declaration styles every participant of its class: the group rule carries the
  // transition options, and it matches an instance by the class token alone.
  const duration = await page.evaluate(async () => {
    const token = getComputedStyle(document.querySelector("#static")).viewTransitionName;
    const transition = document.startViewTransition(() => {
      document.querySelector("#static").textContent = "changed";
    });
    await transition.ready;
    // The pseudo-element itself is reported by participant name; the rule that reaches
    // it was written for the class token, which is what the emitted stylesheet carries.
    const animation = document
      .getAnimations()
      .find((entry) => entry.effect?.pseudoElement === `::view-transition-group(${token})`);
    const value = animation?.effect?.getTiming().duration;
    await transition.finished;
    return value;
  });
  assert.equal(duration, 100, "the group rule that matches by class token applies");

  console.log("View transitions match by class token and name their participants by key.");
} finally {
  await browser.close();
}
