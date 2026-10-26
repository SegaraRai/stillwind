import { definePlugin } from "@stillwind/config";
import { expect, test } from "vite-plus/test";

import { applyEdits } from "../../src/editor/sort.ts";
import { project, theme } from "./support.ts";

test("variant snippets preserve string contexts and group prefixes", async () => {
  const css = `${theme} @stillwind-variant nth-* (value: [nth]) { &:--value(":nth-child({value})") { @slot; } }`;
  for (const input of [
    '"nth-"',
    "'nth-'",
    "`nth-`",
    '{ "hover:": "nth-" }',
    '"[&[data-label=\\\"x\\\"]]:nth-"',
    '"[&[data-label=\\\"[]\\\"]]:nth-"',
  ]) {
    const source = `import { style } from "stillwind/react"; const value = style(${input});`;
    const current = project(source, {}, css);
    try {
      const offset = source.indexOf("nth-") + 4;
      const plain = (await current.query("/workspace/entry.tsx", {
        kind: "completion",
        offset,
      }))!.completions!;
      expect(plain.some((item) => item.label.includes("[]"))).toBe(false);
      const items = (await current.query("/workspace/entry.tsx", {
        kind: "completion",
        offset,
        snippets: true,
      }))!.completions!;
      const item = items.find((item) => item.label.endsWith("nth-[]:"))!;
      expect(item?.snippet).toContain("nth-[${1}]:${0}");
      expect(item.retrigger).toBe(true);
      if (input.startsWith("{")) {
        expect(item.label).toBe("nth-[]:");
      }
      if (input.includes("data-label")) {
        expect(item.newText).toContain(input.includes("[]") ? '\\"[]\\"' : '\\"x\\"');
      }
      const completed = applyEdits(source, [
        {
          ...item,
          newText: item.newText.replace("nth-[]", "nth-[2n]") + "block",
        },
      ]);
      const next = project(completed, {}, css);
      try {
        expect(
          (
            await next.query("/workspace/entry.tsx", {
              kind: "hover",
              offset: completed.indexOf("nth-"),
            })
          )?.hover?.css,
        ).toContain(":nth-child(2n)");
      } finally {
        await next.close();
      }
    } finally {
      await current.close();
    }
  }
});

test("selected completion details describe utilities and variants in their captured snapshot", async () => {
  const source = 'import { style } from "stillwind/react"; const value = style("");';
  const current = project(source);
  try {
    const items = (await current.query("/workspace/entry.tsx", {
      kind: "completion",
      offset: source.indexOf('("') + 2,
    }))!.completions!;
    for (const [label, detail] of [
      ["block", "display: block;"],
      ["hover:", "&:hover"],
    ]) {
      const item = items.find((item) => item.label === label)!;
      const request = {
        kind: "resolve" as const,
        candidate: item.candidate,
        input: item.input,
        snapshot: item.snapshot,
      };
      expect((await current.query("/workspace/entry.tsx", request))?.hover?.detail).toBe(detail);
      expect(
        (
          await current.query("/workspace/entry.tsx", {
            ...request,
            snapshot: "stale",
          })
        )?.hover,
      ).toBeUndefined();
    }
  } finally {
    await current.close();
  }
});

test("style.media arguments complete and preview only media variants", async () => {
  const source =
    'import {style} from "stillwind/react"; const wide = style.media("wide:"); const next = style.media("");';
  const current = project(
    source,
    {},
    `${theme}
@custom-variant wide (@media (width >= 40rem));`,
  );
  try {
    const hover = (await current.query("/workspace/entry.tsx", {
      kind: "hover",
      offset: source.indexOf("wide:") + 1,
    }))!.hover;
    expect(hover?.css).toContain("@media (width >= 40rem /* 640px */)");
    const completions = (await current.query("/workspace/entry.tsx", {
      kind: "completion",
      offset: source.lastIndexOf('("') + 2,
    }))!.completions!;
    const labels = completions.map((item) => item.label);
    expect(labels).toContain("wide:");
    expect(labels).not.toContain("hover:");
    expect(labels).not.toContain("block");
  } finally {
    await current.close();
  }
});

test("style.css calls and inputs preview their output, and template names complete", async () => {
  const source =
    'import {style} from "stillwind/react"; const sheet = style.css("[:host]:flex", { "[button]:": "block" }); const anchored = style.css("p-4", { selector: ":host", template: "shadow" }); const plain = style.css("", { template: "" });';
  const current = project(
    source,
    {},
    `${theme}
@custom-variant shell scope(document) (:host);
@stillwind-css-template default { @stillwind-slot utilities; }
@stillwind-css-template shadow { :host { @stillwind-slot theme; } @stillwind-slot utilities; }`,
  );
  try {
    const hover = async (offset: number) =>
      (await current.query("/workspace/entry.tsx", { kind: "hover", offset }))!.hover?.css;
    expect(await hover(source.indexOf("flex"))).toMatch(/:host \{\n\s+display: flex;/);
    expect(await hover(source.indexOf("block"))).toMatch(/button \{\n\s+display: block;/);
    // The input preview applies the call's selector option and omits the template.
    const input = await hover(source.indexOf("p-4"));
    expect(input).toMatch(/:host \{\n\s+padding:/);
    expect(input).not.toContain("--spacing:");
    // The call preview is the string that the call lowers to.
    const call = await hover(source.indexOf("style.css", source.indexOf("anchored")) + 7);
    expect(call).toMatch(/--spacing: \.25rem\b/);
    expect(call).toMatch(/:host \{\n\s+padding:/);
    expect(await hover(source.indexOf('"shadow"') + 2)).toContain("@stillwind-slot theme;");
    const labels = async (offset: number) =>
      (await current.query("/workspace/entry.tsx", {
        kind: "completion",
        offset,
      }))!.completions!.map((item) => item.label);
    const plain = await labels(source.indexOf('style.css("",') + 11);
    expect(plain).toContain("shell:");
    expect(plain).not.toContain("hover:");
    expect(plain).not.toContain("block");
    expect(await labels(source.indexOf("p-4"))).toContain("block");
    expect(await labels(source.lastIndexOf('template: "') + 11)).toEqual(["default", "shadow"]);
  } finally {
    await current.close();
  }
});

test("plugin catalogs complete dynamic utilities, and hover shows their resolved CSS", async () => {
  const source =
    'import {style} from "stillwind/react"; const icon = style("demo-red"); const next = style("hover:demo-b");';
  const current = project(
    source,
    {
      plugins: [
        definePlugin({
          name: "demo",
          css: () => "@stillwind-dynamic demo-*;",
          resolve: (candidates) =>
            candidates.map((name) => `@utility ${name} { color: ${name.slice(5)}; }`).join(" "),
          catalog: () => ["demo-red", "demo-blue"],
        }),
      ],
    },
    `${theme} @plugin "demo";`,
  );
  try {
    await current.analyze("/workspace/entry.tsx");
    const hover = await current.query("/workspace/entry.tsx", {
      kind: "hover",
      offset: source.indexOf("demo-red"),
    });
    expect(hover?.hover?.css).toContain("color: red");
    const completion = await current.query("/workspace/entry.tsx", {
      kind: "completion",
      offset: source.indexOf("demo-b") + 6,
    });
    const labels = completion!.completions!.map((item) => item.label);
    expect(labels).toContain("hover:demo-blue");
    expect(labels).not.toContain("hover:demo-red");
  } finally {
    await current.close();
  }
});

test("incomplete literals and outer variants retain original completion and hover ranges", async () => {
  const source = 'import {style} from "stillwind/react"; const card=style({"hover:":"bg-br';
  const current = project(source);
  try {
    const items = (await current.query("/workspace/entry.tsx", {
      kind: "completion",
      offset: source.length,
    }))!.completions!;
    const item = items.find((item) => item.label === "bg-brand")!;
    expect(item).toMatchObject({
      candidate: "hover:bg-brand",
      start: source.indexOf("bg-br"),
      end: source.length,
      newText: "bg-brand",
    });
  } finally {
    await current.close();
  }
  const complete =
    'import {style} from "stillwind/react"; const card=style({"hover:":"bg-brand"});';
  const next = project(complete);
  try {
    expect(
      (await next.query("/workspace/entry.tsx", {
        kind: "hover",
        offset: complete.indexOf("bg-brand"),
      }))!.hover?.css,
    ).toContain("&:hover");
  } finally {
    await next.close();
  }
});

test("indexed table values complete and hover as utilities, and sorting keeps entry order", async () => {
  const source =
    'import {style} from "stillwind/react"; declare const kind: "a" | "b"; declare const index: number; const card=style({"hover:":{a:"bg-brand",b:"block"}[kind]}); const list=style(["p-4","flex"][index]);';
  const current = project(source);
  try {
    const end = source.indexOf("bg-brand") + "bg-br".length;
    const items = (await current.query("/workspace/entry.tsx", {
      kind: "completion",
      offset: end,
    }))!.completions!;
    expect(items.find((item) => item.label === "bg-brand")).toMatchObject({
      candidate: "hover:bg-brand",
    });
    expect(
      (await current.query("/workspace/entry.tsx", {
        kind: "hover",
        offset: source.indexOf("bg-brand"),
      }))!.hover?.css,
    ).toContain("&:hover");
    const sorted = applyEdits(
      source,
      (await current.query("/workspace/entry.tsx", {
        kind: "sort",
      }))!.edits!,
    );
    expect(sorted).toContain('["p-4","flex"][index]');
  } finally {
    await current.close();
  }
});

test("empty rule keys retain completion prefixes, hover selectors, and utility grouping", async () => {
  for (const key of ["", ":"]) {
    const name = JSON.stringify(key);
    const incomplete = `import {style} from "stillwind/react"; const card=style({"hover:":{${name}:"bg-br`;
    const current = project(incomplete);
    try {
      const items = (await current.query("/workspace/entry.tsx", {
        kind: "completion",
        offset: incomplete.length,
      }))!.completions!;
      expect(items.find((item) => item.label === "bg-brand")).toMatchObject({
        candidate: "hover:bg-brand",
        start: incomplete.indexOf("bg-br"),
        end: incomplete.length,
        newText: "bg-brand",
      });
    } finally {
      await current.close();
    }
    const source = `import {style} from "stillwind/react"; const card=style({"--z":value,${name}:"flex block","hover:":{${name}:"bg-brand"}});`;
    const complete = project(source, {
      sort: {
        stringLiterals: true,
        arrays: true,
      },
    });
    try {
      expect(
        (await complete.query("/workspace/entry.tsx", {
          kind: "hover",
          offset: source.indexOf("bg-brand"),
        }))!.hover?.css,
      ).toContain("&:hover");
      const sorted = applyEdits(
        source,
        (await complete.query("/workspace/entry.tsx", {
          kind: "sort",
        }))!.edits!,
      );
      expect(sorted).toContain(
        `style({${name}:"block flex","hover:":{${name}:"bg-brand"},"--z":value})`,
      );
    } finally {
      await complete.close();
    }
  }
});

test("namespace imports complete unfinished style calls", async () => {
  const source = 'import * as sw from "stillwind/react"; const value=sw.style("bg-br';
  const current = project(source);
  try {
    expect(
      (await current.query("/workspace/entry.tsx", {
        kind: "completion",
        offset: source.length,
      }))!.completions?.map((item) => item.label),
    ).toContain("bg-brand");
  } finally {
    await current.close();
  }
});

test("aria and data variant wrappers remain utility inputs", async () => {
  const source =
    'import {style} from "stillwind/react"; const value=style({"aria-checked:":"flex block","--x":2,"data-active:":"block"});';
  const current = project(
    source,
    {},
    `${theme} @variant aria-checked { &[aria-checked=true] { @slot; } } @variant data-active { &[data-active] { @slot; } }`,
  );
  try {
    expect(
      applyEdits(
        source,
        (await current.query("/workspace/entry.tsx", {
          kind: "sort",
        }))!.edits!,
      ),
    ).toContain('style({"aria-checked:":"block flex","data-active:":"block","--x":2})');
  } finally {
    await current.close();
  }
});

test("marker classes complete, describe their origins on hover, and sort before utilities", async () => {
  const css = `${theme}
@stillwind-marker menu (modifiable);
@utility card-title { .card & { color: red; } }
`;
  const source =
    'import {style} from "stillwind/react"; const a=style("flex card"); const b=style({"hover:":"card"}); const c=style("car';
  const current = project(source, {}, css);
  try {
    const items = (await current.query("/workspace/entry.tsx", {
      kind: "completion",
      offset: source.length,
      snippets: true,
    }))!.completions!;
    expect(items.find((item) => item.label === "card")).toMatchObject({
      marker: true,
    });
    expect(items.find((item) => item.label === "card-title")?.marker).toBeUndefined();
    const hover = (await current.query("/workspace/entry.tsx", {
      kind: "hover",
      offset: source.indexOf("card"),
    }))!.hover!;
    expect(hover).toMatchObject({
      start: source.indexOf("card"),
      end: source.indexOf("card") + 4,
      css: "",
      documentation:
        "Adds the marker class `card` to the element as written.\n\nSelectors in the `card-title` utility select this class.",
    });
    expect(
      (await current.query("/workspace/entry.tsx", {
        kind: "hover",
        offset: source.indexOf('"card"}'),
      }))!.hover,
    ).toBeUndefined();
    const item = items.find((item) => item.label === "card")!;
    expect(
      (await current.query("/workspace/entry.tsx", {
        kind: "resolve",
        candidate: item.candidate,
        input: item.input,
        snapshot: item.snapshot,
      }))!.hover,
    ).toMatchObject({ css: "", detail: "marker class" });
    const sorted = applyEdits(
      source,
      (await current.query("/workspace/entry.tsx", {
        kind: "sort",
      }))!.edits!,
    );
    expect(sorted).toContain('style("card flex")');
  } finally {
    await current.close();
  }
  const modifier = 'import {style} from "stillwind/react"; const a=style("men';
  const next = project(modifier, {}, css);
  try {
    const items = (await next.query("/workspace/entry.tsx", {
      kind: "completion",
      offset: modifier.length,
      snippets: true,
    }))!.completions!;
    expect(
      items.filter((item) => item.marker).map(({ label, snippet }) => [label, snippet]),
    ).toEqual([
      ["menu", undefined],
      ["menu/", "menu/${1:name}"],
    ]);
  } finally {
    await next.close();
  }
});
