import { readFile, readdir } from "node:fs/promises";

import { expect, test } from "vite-plus/test";

import { BrowserProject } from "../browser/index.ts";
import type { EditorCompletion } from "./snapshot.ts";
import { applyEdits } from "./sort.ts";

const entry = "/workspace/entry.tsx";

/**
 * Built package declarations and TypeScript libraries used to resolve actual producer
 * identities.
 */
const declarations = (async () => {
  const files = new Map<string, string>();
  const core = new URL("../../../core/", import.meta.url);
  files.set(
    "/workspace/node_modules/stillwind/package.json",
    await readFile(new URL("package.json", core), "utf8"),
  );
  for (const name of await readdir(new URL("dist/", core), { recursive: true })) {
    if (name.endsWith(".d.ts")) {
      files.set(
        `/workspace/node_modules/stillwind/dist/${name.replaceAll("\\", "/")}`,
        await readFile(new URL(`dist/${name.replaceAll("\\", "/")}`, core), "utf8"),
      );
    }
  }
  const library = new URL("../../node_modules/typescript-6/lib/", import.meta.url);
  for (const name of await readdir(library)) {
    if (name.endsWith(".d.ts")) {
      files.set(
        `/node_modules/typescript/lib/${name}`,
        await readFile(new URL(name, library), "utf8"),
      );
    }
  }
  const motion = await readFile(new URL("css/builtin-utilities/tw-motion.css", core), "utf8");
  const variants = await readFile(new URL("css/builtin-variants/tw-conditional.css", core), "utf8");
  return {
    files,
    css: `${motion}\n${variants}\n
@theme { --duration-normal:180ms; --ease-soft:ease-in; --breakpoint-md:40rem; }
@utility block { display:block; } @utility opacity-* { opacity:calc(--value(integer) / 100); }
@stillwind-variant hover { &:hover { @slot; } }
@stillwind-variant custom-media { @media print { @slot; } }
@stillwind-variant custom-supports { @supports (display:grid) { @slot; } }
:root { @stillwind-slot theme; } @stillwind-slot utilities; @stillwind-slot keyframes;`,
  };
})();

/**
 * Creates an independent browser project with declarations for actual producer recognition.
 *
 * @param source - Original TSX content stored at the shared entry path.
 * @param dependencies - Additional files keyed by absolute virtual path.
 * @param cssSuffix - CSS appended to the shared motion and variant theme.
 * @returns A project whose lifecycle the caller must end with `close()`.
 * @throws If the shared declaration or stylesheet reads fail.
 */
async function project(source: string, dependencies: Record<string, string> = {}, cssSuffix = "") {
  const shared = await declarations;
  return new BrowserProject({
    root: "/workspace",
    files: new Map([
      ...shared.files,
      [entry, source],
      ["/workspace/theme.css", shared.css + cssSuffix],
      ...Object.entries(dependencies),
    ]),
    config: {
      css: "theme.css",
      files: ["*.tsx"],
      tsconfig: false,
    },
  });
}

/**
 * Queries hover at the start of an occurrence in the original entry source.
 *
 * @param current - Project whose entry buffer is queried.
 * @param source - Original text that must match that entry buffer.
 * @param text - Exact substring used to locate the query position.
 * @param occurrence - Nonnegative integer index of the match, starting at zero.
 * @returns Hover CSS, or undefined when the editor cannot supply it.
 * @throws If the occurrence is absent or the project query fails.
 */
async function hover(current: BrowserProject, source: string, text: string, occurrence = 0) {
  let offset = -1;
  for (let index = 0; index <= occurrence; index++) {
    offset = source.indexOf(text, offset + 1);
  }
  expect(offset).toBeGreaterThanOrEqual(0);
  return (
    await current.query(entry, {
      kind: "hover",
      offset,
    })
  )?.hover?.css;
}

/**
 * Queries completion at the start of an occurrence in the original entry source.
 *
 * @param current - Project whose entry buffer is queried.
 * @param source - Original text that must match that entry buffer.
 * @param text - Exact substring used to locate the query position.
 * @param occurrence - Nonnegative integer index of the match, starting at zero.
 * @returns Completion items, or an empty array when the editor cannot supply them.
 * @throws If the occurrence is absent or the project query fails.
 */
async function complete(current: BrowserProject, source: string, text: string, occurrence = 0) {
  let offset = -1;
  for (let index = 0; index <= occurrence; index++) {
    offset = source.indexOf(text, offset + 1);
  }
  expect(offset).toBeGreaterThanOrEqual(0);
  return (
    (
      await current.query(entry, {
        kind: "completion",
        offset,
      })
    )?.completions ?? []
  );
}

/**
 * Resolves an entry completion using its captured literal position and fingerprint.
 *
 * @param current - Project whose entry is queried for the current snapshot.
 * @param item - Entry completion whose candidate, input boundary, and snapshot are passed unchanged.
 * @returns The editor result, or undefined if the entry is unavailable. A stale capture fingerprint
 * omits hover output.
 * @throws If the project query fails.
 */
function resolve(current: BrowserProject, item: EditorCompletion) {
  return current.query(entry, {
    kind: "resolve",
    candidate: item.candidate,
    input: item.input,
    snapshot: item.snapshot,
  });
}

test("keyframes inputs and animation options use distinct registries and definition conditions", async () => {
  const source = `import {style} from "stillwind/react";
const 日本語="🦀";
const frames = style.keyframes({from:"opacity-0 duration-180",to:"opacity-100","custom-media:":{from:"opacity-100","custom-supports:":{to:"opacity-0"}}});
const value = style(frames("duration-normal ease-soft both"));
console.log(frames);`;
  const current = await project(source);
  try {
    const graph = await current.analyze(entry);
    expect(
      graph.batches
        .flatMap((batch) => batch.reports)
        .filter((report) => report.severity === "error"),
    ).toEqual([]);
    const frame = await hover(current, source, "duration-180");
    expect(frame).toContain("from {");
    expect(frame).toContain("transition-duration:");
    expect(frame).not.toContain("animation-duration:");
    const option = await hover(current, source, "duration-normal");
    expect(option).toContain("animation-duration: var(--duration-normal)");
    expect(option).not.toContain("transition-duration:");
    const body = await complete(current, source, "opacity-0");
    expect(body.some((item) => item.label.endsWith(":"))).toBe(false);
    expect(body.some((item) => item.label === "both")).toBe(false);
    const options = await complete(current, source, "duration-normal");
    expect(options.map((item) => item.label)).toEqual(
      expect.arrayContaining(["duration-normal", "ease-soft", "both"]),
    );
    expect(options.some((item) => item.label === "block" || item.label.endsWith(":"))).toBe(false);
    const duration = options.find((item) => item.label === "duration-normal")!;
    expect((await resolve(current, duration))?.hover?.css).toBe(option);
    const branches = await complete(current, source, "custom-media:");
    expect(branches.map((item) => item.label)).toEqual(
      expect.arrayContaining(["custom-media:", "custom-supports:", "motion-reduce:", "md:"]),
    );
    expect(branches.some((item) => item.label === "hover:" || item.label.startsWith("@"))).toBe(
      false,
    );
    const definition = await hover(current, source, "style.keyframes");
    expect(definition?.match(/@keyframes /g)).toHaveLength(3);
    expect(definition).toContain("@media print");
    expect(definition).toContain("@supports (display: grid)");
    const use = await hover(current, source, 'frames("');
    expect(use).toContain("animation-name:");
    expect(use?.match(/@keyframes /g)).toHaveLength(3);
    expect(await hover(current, source, "frames);", 0)).toBe(definition);
  } finally {
    await current.close();
  }
});

test("view-transition group options and phase utilities keep pseudo-elements and allowed variants", async () => {
  const source = `import {style} from "stillwind/react";
const transition = style.viewTransition({group:"duration-normal both motion-reduce:duration-0",old:"motion-reduce:opacity-0",new:"opacity-100",imagePair:"block"});
declare const flag:boolean;
const value=style("block [color:red]",flag&&"opacity-0",flag?transition.for("hero"):null); console.log(value);`;
  const current = await project(source);
  try {
    const group = await hover(current, source, "duration-normal");
    expect(group).toContain("::view-transition-group");
    expect(group).toContain("animation-duration:");
    const phase = await hover(current, source, "motion-reduce:opacity-0");
    expect(phase).toContain("::view-transition-old");
    expect(phase).toContain("@media (prefers-reduced-motion: reduce)");
    const options = await complete(current, source, "duration-normal");
    expect(options.map((item) => item.label)).toEqual(
      expect.arrayContaining(["both", "duration-normal", "motion-reduce:"]),
    );
    expect(options.some((item) => item.label === "hover:" || item.label === "block")).toBe(false);
    const utilities = await complete(current, source, "block");
    expect(utilities.map((item) => item.label)).toContain("block");
    expect(
      utilities.some(
        (item) => item.label === "both" || item.label === "hover:" || item.label.startsWith("@"),
      ),
    ).toBe(false);
    const item = options.find((item) => item.label === "duration-normal")!;
    expect((await resolve(current, item))?.hover?.css).toBe(group);
    const definition = await hover(current, source, "style.viewTransition");
    expect(definition).toContain("::view-transition-image-pair");
    expect(definition).toContain("::view-transition-new");
    const use = await hover(current, source, 'transition.for("');
    expect(use).toContain("view-transition-name:");
    expect(use).toContain("::view-transition-group");
    expect(use).not.toContain("color:");
    const graph = await current.analyze(entry);
    expect(graph.generation).toBe("complete");
    expect(graph.unverified).toEqual([]);
    expect(
      graph.batches
        .flatMap((batch) => batch.reports)
        .filter((report) => report.severity === "error"),
    ).toEqual([]);
    const emitted = graph.modules.map((module) => module.css).join("\n");
    const whole = await hover(current, source, 'style("block');
    const slots = [...(use?.matchAll(/var\((--__transition_class_\d+),/g) ?? [])].map(
      (match) => match[1],
    );
    expect(slots.length).toBeGreaterThan(0);
    for (const slot of slots) {
      expect(use).toContain(`${slot}: __transition_transition`);
      expect(whole).toContain(`${slot}: __transition_transition`);
    }
    expect(use).toContain("view-transition-name: __transition_transition-hero");
    expect(use).toContain("::view-transition-old(.__transition_transition)");
    expect(emitted).toMatch(/view-transition-name:sw-vt-/);
  } finally {
    await current.close();
  }
});

test("ref projections expose only their own declarations after class merging", async () => {
  const source = `import {style} from "stillwind/react";
const panel=style.ref("panel"); const other=style.ref("other");
const value=style("block",panel.asAnchor(),panel.asContainer("inline-size"),panel.positionAnchor(),style.anchorScope([other]));
const empty=style(style.anchorScope([]));
const child=style({[panel("hover:")]:"block"});`;
  const current = await project(source);
  try {
    const graph = await current.analyze(entry);
    expect(
      graph.batches
        .flatMap((batch) => batch.reports)
        .filter((report) => report.severity === "error"),
    ).toEqual([]);
    const anchor = await hover(current, source, "asAnchor");
    expect(anchor).toContain("&:where(.__ref_panel)");
    expect(anchor).toContain("anchor-name: --__anchor_panel");
    expect(anchor).not.toMatch(/anchor-scope|container-name|display:/);
    const container = await hover(current, source, "asContainer");
    expect(container).toContain("container-type: inline-size");
    expect(container).toContain("container-name: __container_panel");
    expect(container).not.toMatch(/anchor-name|display:/);
    const position = await hover(current, source, "positionAnchor");
    expect(position).toContain("position-anchor: --__anchor_panel");
    expect(position).not.toContain(":where(");
    const scope = await hover(current, source, "anchorScope");
    expect(scope).toContain("anchor-scope: --__anchor_other");
    expect(scope).not.toContain("--__anchor_panel");
    expect(await hover(current, source, "anchorScope", 1)).toBeUndefined();
    const computed = await hover(current, source, 'block"});');
    expect(computed).toContain(".__ref_panel");
    expect(computed).toContain(":hover");
    const whole = await hover(current, source, 'style("block"');
    expect(whole).toContain("display: block");
    expect(whole).toContain("anchor-name:");
    expect(whole).toContain("container-name:");
  } finally {
    await current.close();
  }
});

test("imported definitions, composed styles, and finite uses contribute their actual CSS", async () => {
  const source = `import {style} from "stillwind/react"; import {first, second, base} from "./assets";
declare const flag:boolean;
const selected=flag?first:second;
const value=style(base,{"hover:":selected("duration-normal")});
import * as assets from "./assets"; const single=style(assets.first("both"));
console.log(value);`;
  const dependency = `import {style} from "stillwind/react";
export const first=style.keyframes({from:"opacity-0",to:"opacity-100"});
export const second=style.keyframes({from:"opacity-100",to:"opacity-0"});
export const base=style("block");`;
  const current = await project(source, {
    "/workspace/assets.tsx": dependency,
  });
  try {
    const graph = await current.analyze(entry);
    expect(
      graph.batches
        .flatMap((batch) => batch.reports)
        .filter((report) => report.severity === "error"),
    ).toEqual([]);
    const use = await hover(current, source, 'selected("');
    expect(use).toContain("&:hover");
    expect(use?.match(/@keyframes /g)).toHaveLength(2);
    const option = await hover(current, source, "duration-normal");
    expect(option).toContain("&:hover");
    expect(option?.match(/animation-name:/g)).toHaveLength(2);
    const whole = await hover(current, source, "style(base");
    expect(whole).toContain("display: block");
    expect(whole?.match(/@keyframes /g)).toHaveLength(2);
    expect(await hover(current, source, "value);", 0)).toBe(whole);
    const qualified = await hover(current, source, 'first("both"');
    expect(qualified).toContain("animation-name:");
    expect(qualified?.match(/@keyframes /g)).toHaveLength(1);
    for (const binding of ["first", "second"]) {
      expect(whole).toContain(`animation-name: __keyframes_${binding}`);
      expect(whole).toContain(`@keyframes __keyframes_${binding}`);
    }
    expect(qualified).toContain("animation-name: __keyframes_first");
  } finally {
    await current.close();
  }
});

test("display names follow original identities through aliases and distinguish equal labels", async () => {
  const source = `import {style} from "stillwind/react";
import {frames as left} from "./a"; import {frames as right} from "./b"; import * as assets from "./a";
const first=style(left("both")); const second=style(right("both")); const third=style(assets.frames("both"));
const authored=style("authored");`;
  const current = await project(
    source,
    {
      "/workspace/a.tsx":
        'import {style} from "stillwind/react"; export const frames=style.keyframes({label:"fade",from:"opacity-0"});',
      "/workspace/b.tsx":
        'import {style} from "stillwind/react"; export const frames=style.keyframes({label:"fade",to:"opacity-100"});',
    },
    `@utility authored { animation-name:spin; container-name:manual; --user-name:__keyframes_fade; content:"sw-kf-kept"; background-image:url("sw-kf-kept.svg"); }`,
  );
  try {
    const right = await hover(current, source, 'right("');
    const left = await hover(current, source, 'left("');
    expect(left).toContain("animation-name: __keyframes_fade");
    expect(left).toContain("@keyframes __keyframes_fade {");
    expect(right).toContain("animation-name: __keyframes_fade_2");
    expect(right).toContain("@keyframes __keyframes_fade_2 {");
    expect(await hover(current, source, 'assets.frames("')).toBe(left);
    expect(await hover(current, source, 'right("')).toBe(right);
    const authored = await hover(current, source, '"authored"');
    expect(authored).toContain("animation-name: spin");
    expect(authored).toContain("container-name: manual");
    expect(authored).toContain("--user-name: __keyframes_fade");
    expect(authored).toContain('content: "sw-kf-kept"');
    expect(authored).toContain("sw-kf-kept.svg");
  } finally {
    await current.close();
  }
});

test("dynamic transition names expose runtime keys and retain distinct class and name slots", async () => {
  const source = `import {style} from "stillwind/react";
const card=style.viewTransition({label:"card",old:"opacity-0"}); declare const id:string; declare const flag:boolean;
const direct=style(card.for(id)); const conditional=style("block",flag&&{"hover:":card.for(id)});`;
  const current = await project(source);
  try {
    const direct = (
      await current.query(entry, {
        kind: "hover",
        offset: source.indexOf("card.for(id)"),
      })
    )?.hover;
    expect(direct?.runtimeNames).toEqual(["id"]);
    expect(direct?.css).toContain("view-transition-class: __transition_card");
    expect(direct?.css).not.toContain("view-transition-name:");
    const guarded = (
      await current.query(entry, {
        kind: "hover",
        offset: source.lastIndexOf("card.for(id)"),
      })
    )?.hover;
    expect(guarded?.runtimeNames).toEqual(["id"]);
    expect(guarded?.css).toContain("view-transition-name: var(--__transition_name_1)");
    expect(guarded?.css).toContain("--__transition_class_1: __transition_card");
    expect(guarded?.css).toContain("view-transition-class: var(--__transition_class_1, )");
    expect(guarded?.css).toContain("&:hover");
    const whole = await hover(current, source, 'style("block');
    expect(whole).toContain("view-transition-name: var(--__transition_name_1)");
    expect(whole).toContain("--__transition_class_1: __transition_card");
  } finally {
    await current.close();
  }
});

test("escaped literals and incomplete definition keys retain roles and original edit ranges", async () => {
  const source = String.raw`import {style as s} from "stillwind/react"; const 日本語="🦀";
const frames=s.keyframes({from:"opacity-0"}); const value=s({"hover:":frames("duration-n\u006frmal")});`;
  const current = await project(source);
  try {
    const items = await complete(current, source, String.raw`duration-n\u006frmal`);
    const item = items.find((item) => item.label === "duration-normal")!;
    expect(source.slice(item.start, item.end)).toBe(String.raw`duration-n\u006frmal`);
    expect(applyEdits(source, [item])).toContain('frames("duration-normal")');
    const css = (await resolve(current, item))?.hover?.css;
    expect(css).toContain("&:hover");
    expect(css).toContain("animation-duration:");
  } finally {
    await current.close();
  }
  const incomplete =
    'import {style as s} from "stillwind/react"; const frames=s.keyframes({"moti":{from:"opacity-0"}}); const other={keyframes(x:unknown){return x;}}; other.keyframes({from:"block"});';
  const next = await project(incomplete);
  try {
    const items = (
      await next.query(entry, {
        kind: "completion",
        offset: incomplete.indexOf("moti") + 4,
      })
    )?.completions;
    expect(items?.map((item) => item.label)).toContain("motion-reduce:");
    expect(await complete(next, incomplete, "block")).toEqual([]);
  } finally {
    await next.close();
  }
});

test("resolve rejects changed dependencies and themes, and invalid sites have no partial preview", async () => {
  const source = `import {style} from "stillwind/react"; import {frames} from "./assets";
const value=style(frames("duration-normal"));`;
  const dependency =
    'import {style} from "stillwind/react"; export const frames=style.keyframes({from:"opacity-0",to:"opacity-100"});';
  const current = await project(source, {
    "/workspace/assets.tsx": dependency,
  });
  const changed = await project(source, {
    "/workspace/assets.tsx": dependency.replace('from:"opacity-0"', 'from:"opacity-50"'),
  });
  const rethemed = await project(
    source,
    {
      "/workspace/assets.tsx": dependency,
    },
    "@theme { --duration-normal:240ms; }",
  );
  try {
    const item = (await complete(current, source, "duration-normal")).find(
      (item) => item.label === "duration-normal",
    )!;
    expect((await resolve(current, item))?.hover).toBeDefined();
    expect((await resolve(changed, item))?.hover).toBeUndefined();
    expect((await resolve(rethemed, item))?.hover).toBeUndefined();
  } finally {
    await Promise.all([current.close(), changed.close(), rethemed.close()]);
  }
  const invalid = `import {style} from "stillwind/react";
const bad=style.keyframes({from:"opacity-0 hover:opacity-100"}); const unused=style.keyframes({to:"opacity-100"});
const value=style("block",bad("duration-normal")); const wrong=style.viewTransition({group:"block"});`;
  const rejected = await project(invalid);
  try {
    expect(await hover(rejected, invalid, "style.keyframes")).toBeUndefined();
    expect(await hover(rejected, invalid, 'style("block"')).toBeUndefined();
    expect(await hover(rejected, invalid, "style.viewTransition")).toBeUndefined();
    expect(await hover(rejected, invalid, "style.keyframes", 1)).toContain("to {");
    expect(await hover(rejected, invalid, "hover:opacity-100")).toBeUndefined();
  } finally {
    await rejected.close();
  }
});

test("source sorting leaves animation option order and keyframe bodies intact", async () => {
  const source = `import {style} from "stillwind/react";
const frames=style.keyframes({from:"opacity-0 duration-180"});
const value=style(frames("duration-300 duration-180 both ease-soft"),"opacity-0 block");
const transition=style.viewTransition({group:"duration-300 duration-180",old:"opacity-0 block"});`;
  const current = await project(source);
  try {
    const sorted = applyEdits(
      source,
      (
        await current.query(entry, {
          kind: "sort",
        })
      )?.edits ?? [],
    );
    expect(sorted).toContain('frames("duration-300 duration-180 both ease-soft")');
    expect(sorted).toContain('from:"opacity-0 duration-180"');
    expect(sorted).toContain('old:"opacity-0 block"');
    expect(sorted).toContain('"block opacity-0"');
  } finally {
    await current.close();
  }
});
