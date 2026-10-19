import { readFile } from "node:fs/promises";
import { createRequire } from "node:module";
import path from "node:path";
import { pathToFileURL } from "node:url";

import { loadCssGraph } from "@stillwind/host";
import { NativeProjectSession } from "@stillwind/native";
import { chromium, type Browser, type Page } from "playwright";
import plugin from "tailwindcss/plugin";
import { afterAll, beforeAll, describe, expect, test } from "vite-plus/test";

const require = createRequire(import.meta.url);
const root = path.resolve(import.meta.dirname, "..");
const { compile } = require("tailwindcss") as typeof import("tailwindcss");
const tailwindRoot = path.dirname(require.resolve("tailwindcss/package.json"));
const typography = require("@tailwindcss/typography") as unknown;
const tailwindIconify = (
  (await import(pathToFileURL(require.resolve("@iconify/tailwind4")).href)) as {
    default: unknown;
  }
).default;

/** A plugin, stylesheet, and markup that Tailwind and Stillwind must style the same way. */
interface ComparisonCase {
  name: string;
  /** Tailwind plugin export of the `@plugin` statement. */
  extension: unknown;
  candidates: string[];
  /** CSS before and after the `@plugin` statement. */
  before?: string;
  after?: string;
  /** Places the `@plugin` statement before the framework import. */
  pluginFirst?: boolean;
  /** Adds Tailwind's preflight, which Stillwind's framework import always contains. */
  preflight?: boolean;
  /** Imports both frameworks without their default theme. */
  themeless?: boolean;
  /**
   * Markup that contains `#target`, given the class attribute of the candidates and the class
   * attribute of each `style()` call.
   */
  html?: (classes: string, calls: string[]) => string;
  /** Element that receives focus before the measurement. */
  focus?: string;
  /** Computed properties of `#target`. */
  properties: string[];
  /** Text that neither stylesheet may contain. */
  absent?: string;
  /**
   * Styles each candidate in its own `style()` call, for markup that gives each candidate its
   * own element. Otherwise one call styles the candidates together, as one class attribute does.
   */
  separate?: boolean;
}

/** A generated stylesheet and the class attribute for the case's candidates. */
interface Output {
  css: string;
  classes: string;
  /** Class attribute of each `style()` call, in candidate order, including marker classes. */
  calls: string[];
}

const box = (classes: string) =>
  `<div style="width:200px;height:120px;font-size:16px"><span id="target" class="${classes}"></span></div>`;

const red = plugin((api) =>
  api.addUtilities({
    ".probe": {
      color: "rgb(255, 0, 0)",
    },
  }),
);
const child = plugin((api) => {
  api.addUtilities({
    ".probe": {
      "& > span": {
        color: "rgb(255, 0, 0)",
      },
    },
  });
  api.matchUtilities(
    {
      tone: (value) => ({
        "& > span": {
          color: value,
        },
      }),
    },
    {
      values: {
        blue: "rgb(0, 0, 255)",
      },
    },
  );
});
const focusParent = (classes: string) =>
  `<div id="parent" tabindex="0" class="${classes}"><span id="target">x</span></div>`;
const sized = plugin((api) =>
  api.matchUtilities(
    {
      probe: (value, { modifier }) => ({
        width: value,
        "--modifier": modifier ?? "none",
      }),
    },
    {
      type: "length",
      values: {
        one: "1px",
      },
      modifiers: "any",
    },
  ),
);

const cases: ComparisonCase[] = [
  {
    name: "diff 1: a stylesheet utility after @plugin overrides the plugin",
    extension: red,
    candidates: ["probe"],
    after: "@utility probe { color: rgb(0, 0, 255); }",
    properties: ["color"],
  },
  {
    name: "diff 1: a stylesheet utility before @plugin overrides the plugin",
    extension: red,
    candidates: ["probe"],
    before: "@utility probe { color: rgb(0, 0, 255); }",
    properties: ["color"],
  },
  ...[false, true].flatMap((pluginFirst) =>
    [
      ["icon-[lucide--home]", "size-full"],
      ["size-full", "icon-[lucide--home]"],
    ].map((candidates): ComparisonCase => ({
      name: `diff 2: size-full overrides @iconify/tailwind4 (${candidates.join(" ")}, plugin first: ${pluginFirst})`,
      extension: tailwindIconify,
      candidates,
      pluginFirst,
      properties: ["width", "height", "display"],
    })),
  ),
  {
    name: "diff 4: focus on a static utility selects the parent before its child selector",
    extension: child,
    candidates: ["focus:probe"],
    html: focusParent,
    focus: "#parent",
    properties: ["color"],
  },
  {
    name: "diff 4: focus on a dynamic utility selects the parent before its child selector",
    extension: child,
    candidates: ["focus:tone-blue"],
    html: focusParent,
    focus: "#parent",
    properties: ["color"],
  },
  {
    name: "diff 5: every class of a selector registers a utility",
    extension: plugin((api) =>
      api.addUtilities({
        ".outer .inner": {
          color: "rgb(255, 0, 0)",
        },
        "li:not(.ghost) .item": {
          width: "7px",
        },
      }),
    ),
    candidates: ["inner", "item"],
    html: (classes) =>
      `<div class="outer"><ul><li><span id="target" style="display:block" class="${classes}"></span></li></ul></div>`,
    properties: ["color", "width"],
  },
  {
    name: "diff 6: plugin bodies expand @apply",
    extension: plugin((api) =>
      api.addUtilities({
        ".card": {
          "@apply p-4": {},
          "& > span": {
            "@apply m-2": {},
          },
        },
      }),
    ),
    candidates: ["card"],
    html: (classes) => `<div id="target" class="${classes}"><span>x</span></div>`,
    properties: ["padding-top"],
  },
  {
    name: "diffs 7-10: callbacks receive decoded arbitrary values and modifiers",
    extension: sized,
    separate: true,
    candidates: ["probe-[calc(1px+2px)]", "probe-one/[foo_bar]"],
    html: (classes) =>
      classes
        .split(" ")
        .map((name, index) => `<span id="${index ? "other" : "target"}" class="${name}"></span>`)
        .join(""),
    properties: ["width"],
  },
  {
    name: "diffs 7-10: a modifier with underscores decodes to spaces",
    extension: sized,
    candidates: ["probe-one/[foo_bar]"],
    properties: ["width", "--modifier"],
  },
  {
    name: "diffs 7-10: variables keep underscores, and type hints reach typed callbacks",
    extension: sized,
    separate: true,
    candidates: ["probe-[var(--my_width)]", "probe-(length:--my-size)"],
    before: "@theme { --my_width: 37px; }",
    html: (classes) =>
      `<span id="target" style="display:inline-block" class="${classes.split(" ")[0]}"></span><span style="--my-size:5px;display:inline-block" class="${classes.split(" ")[1]}"></span>`,
    properties: ["width"],
  },
  {
    name: "diffs 7-10: Tailwind's type inference accepts a length calculation",
    extension: sized,
    candidates: ["probe-[calc(100%_-_1rem)]"],
    html: (classes) =>
      `<div style="width:200px"><span id="target" style="display:inline-block" class="${classes}"></span></div>`,
    properties: ["width"],
  },
  {
    name: "matchVariant: named values and the DEFAULT root",
    extension: plugin((api) => {
      api.addUtilities({
        ".probe": {
          color: "rgb(255, 0, 0)",
        },
      });
      api.matchVariant("state", (value) => `&[data-state="${value}"]`, {
        values: {
          DEFAULT: "on",
          open: "open",
        },
      });
    }),
    candidates: ["state:probe", "state-open:probe"],
    html: (classes) => `<span id="target" data-state="on" class="${classes}"></span>`,
    properties: ["color"],
  },
  {
    name: "diff 12: important: true makes utilities win over unlayered CSS",
    extension: plugin(() => {}, {
      important: true,
    }),
    candidates: ["p-4"],
    html: (classes) =>
      `<style>#target{padding:1px}</style><div id="target" class="${classes}"></div>`,
    properties: ["padding-top"],
  },
  {
    name: "diff 12: an important selector limits utilities to its descendants",
    extension: plugin(() => {}, {
      important: "#app",
    }),
    candidates: ["p-4"],
    html: (classes) =>
      `<div id="app"><div id="target" class="${classes}"></div></div><div id="outside" class="${classes}"></div>`,
    properties: ["padding-top"],
  },
  {
    name: "diff 12: an important selector leaves elements outside it unstyled",
    extension: plugin(() => {}, {
      important: "#app",
    }),
    candidates: ["p-4"],
    html: (classes) => `<div id="target" class="${classes}"></div>`,
    properties: ["padding-top"],
  },
  {
    name: "diff 13: theme.container adds centering, padding, and screens",
    extension: plugin(() => {}, {
      theme: {
        container: {
          center: true,
          padding: {
            DEFAULT: "1rem",
            sm: "2rem",
          },
          screens: {
            sm: "600px",
            md: "900px",
          },
        },
      },
    }),
    candidates: ["container"],
    html: (classes) => `<div id="target" class="${classes}">x</div>`,
    properties: ["margin-left", "padding-left", "max-width", "width"],
  },
  {
    name: "config(): bracket paths read the theme, and unknown keys return the fallback",
    extension: plugin(
      (api) =>
        api.addUtilities({
          ".probe": {
            "--a": String(api.config("theme[colors][brand]", "missing")),
            "--b": String(api.config("custom[one]", "missing")),
          },
        }),
      {
        theme: {
          colors: {
            brand: "red",
          },
        },
        custom: {
          one: "found",
        },
      } as never,
    ),
    candidates: ["probe"],
    properties: ["--a", "--b"],
  },
  {
    name: "diff 14: unused utilities contribute no keyframes",
    extension: plugin((api) =>
      api.addUtilities({
        ".spin": {
          animation: "unused-spin 1s",
          "@keyframes unused-spin": {
            to: {
              opacity: "0",
            },
          },
        },
      }),
    ),
    candidates: ["p-4"],
    properties: ["padding-top"],
    absent: "unused-spin",
  },
  ...[
    {
      name: "fontFamily.sans sets the document font and its settings",
      html: box,
    },
    {
      name: "fontFamily.mono sets the font and settings of code",
      html: (classes: string) => `<code id="target" class="${classes}">x</code>`,
    },
    {
      name: "fontFamily.sans leaves an ordinary --font-sans definition in place",
      html: box,
      after: "@theme { --font-sans: Georgia; }",
    },
    {
      name: "fontFamily.sans sets the document font without the default theme",
      html: box,
      themeless: true,
    },
    {
      name: "fontFamily.sans leaves the preflight font for an ordinary --font-sans without the default theme",
      html: box,
      after: "@theme { --font-sans: Georgia; }",
      themeless: true,
    },
    {
      name: "fontFamily.mono sets the font of code without the default theme",
      html: (classes: string) => `<code id="target" class="${classes}">x</code>`,
      themeless: true,
    },
  ].map((entry): ComparisonCase => ({
    ...entry,
    extension: plugin(() => {}, {
      theme: {
        extend: {
          fontFamily: {
            sans: [
              "Inter, sans-serif",
              {
                fontFeatureSettings: '"cv11"',
              },
            ],
            mono: [
              "Fira Code, monospace",
              {
                fontVariationSettings: '"wght" 450',
                fontFeatureSettings: '"zero"',
              },
            ],
          },
        },
      },
    }),
    candidates: ["block"],
    preflight: true,
    properties: ["font-family", "font-feature-settings", "font-variation-settings"],
  })),
  {
    name: "marker classes: typography styles the descendants of prose",
    extension: typography,
    candidates: ["prose"],
    html: (_, [article]) => `<article class="${article}"><p id="target">x</p></article>`,
    properties: ["margin-top", "color"],
  },
  {
    name: "marker classes: not-prose from another call excludes descendants of prose",
    extension: typography,
    separate: true,
    candidates: ["prose", "not-prose"],
    html: (_, [article, excluded]) =>
      `<article class="${article}"><div class="${excluded}"><p id="target">x</p></div></article>`,
    properties: ["margin-top", "color"],
  },
];

/**
 * Builds a case with Tailwind's compiler and its theme and utilities.
 * @param entry - Comparison case with a Tailwind plugin.
 * @returns Tailwind's CSS and class names.
 */
async function tailwind(entry: ComparisonCase): Promise<Output> {
  const imports = `${entry.themeless ? "" : '@import "tailwindcss/theme.css" layer(theme); '}${entry.preflight ? '@import "tailwindcss/preflight.css" layer(base); ' : ""}@import "tailwindcss/utilities.css" layer(utilities);`;
  const statement = `${entry.before ?? ""} @plugin "probe"; ${entry.after ?? ""}`;
  const compiler = await compile(
    entry.pluginFirst ? `${statement} ${imports}` : `${imports} ${statement}`,
    {
      base: root,
      loadStylesheet: async (id) => {
        const file = path.join(tailwindRoot, id.replace("tailwindcss/", ""));
        return {
          path: file,
          base: path.dirname(file),
          content: await readFile(file, "utf8"),
        };
      },
      loadModule: async () => ({
        path: root,
        base: root,
        module: entry.extension as never,
      }),
    },
  );
  return {
    css: compiler.build(entry.candidates),
    classes: entry.candidates.join(" "),
    calls: entry.separate ? entry.candidates : [entry.candidates.join(" ")],
  };
}

/**
 * Builds a case with Stillwind's host, its Tailwind plugin adapter, and the native compiler.
 * @param entry - Comparison case.
 * @returns The project stylesheet and the generated class names.
 */
async function stillwind(entry: ComparisonCase): Promise<Output> {
  const statement = `${entry.before ?? ""} @plugin "probe"; ${entry.after ?? ""}`;
  const imports = entry.themeless
    ? '@layer properties, theme, base, components, utilities; @import "stillwind/preflight.css" layer(base); @import "stillwind/utilities.css" layer(utilities); @import "stillwind/builtins.css"; @import "stillwind/templates.css"; @layer theme { :root, :host { @stillwind-slot theme; } }'
    : '@import "stillwind";';
  const css = entry.pluginFirst ? `${statement} ${imports}` : `${imports} ${statement}`;
  const file = path.join(root, "comparison.css");
  const loaded = await loadCssGraph(
    file,
    {},
    {
      resolve: async (request, importer) => createRequire(importer).resolve(request),
      read: async (id) => (id === file ? css : readFile(id, "utf8")),
      documentUri: (id) => pathToFileURL(id).href,
      sourceMapSource: (id) => id,
      watch: () => {},
      plugins: {
        load: async () => entry.extension as never,
        resolve: async () => undefined,
        read: (id) => readFile(id, "utf8"),
      },
    },
    undefined,
    undefined,
    {
      root,
    },
  );
  for (const candidate of entry.candidates) {
    await loaded.plugins?.query(candidate);
  }
  const session = new NativeProjectSession(loaded.plugins?.replay() ?? loaded.loader, false);
  const id = path.join(root, "comparison.tsx");
  const output = session.compile([
    {
      kind: "direct",
      moduleId: id,
      document: {
        uri: pathToFileURL(id).href,
        code: `import { style } from "stillwind/react"; ${(entry.separate
          ? entry.candidates
          : [entry.candidates.join(" ")]
        )
          .map(
            (candidate, index) => `export const c${index} = style(${JSON.stringify(candidate)});`,
          )
          .join(" ")}`,
        format: "jsx",
        sourceMapSource: id,
      },
      syntax: {
        language: "tsx",
        moduleKind: "unambiguous",
      },
    },
  ]);
  const [module] = output.modules;
  expect(module.diagnostics.map((diagnostic) => diagnostic.message)).toEqual([]);
  return {
    css: session.css().css,
    classes: module.classes.map((generated) => generated.className).join(" "),
    // Generated classes omit the marker classes that a call adds as written.
    calls: [...module.code!.matchAll(/className:"([^"]*)"/g)].map((match) => match[1]),
  };
}

/**
 * Measures `#target` in Chrome. A shared base layer removes default margins and uses
 * `border-box`, because only Stillwind's stylesheet contains a reset.
 * @param page - Browser page.
 * @param entry - Comparison case.
 * @param output - Generated stylesheet and classes.
 * @returns The computed values of the case's properties.
 */
async function measure(page: Page, entry: ComparisonCase, output: Output): Promise<string[]> {
  await page.setViewportSize({
    width: 1000,
    height: 600,
  });
  await page.setContent(
    `<style>@layer base{*,::before,::after{box-sizing:border-box;margin:0;padding:0}}</style><style>${output.css}</style>${(entry.html ?? box)(output.classes, output.calls)}`,
  );
  if (entry.focus) {
    await page.locator(entry.focus).focus();
  }
  return page
    .locator("#target")
    .evaluate(
      (element, properties) =>
        properties.map((name) => getComputedStyle(element).getPropertyValue(name).trim()),
      entry.properties,
    );
}

describe("Tailwind plugin comparison", () => {
  let browser: Browser;
  let page: Page;
  beforeAll(async () => {
    browser = await chromium.launch({
      channel: "chrome",
      headless: true,
    });
    page = await browser.newPage();
  });
  afterAll(async () => {
    await browser?.close();
  });

  test.for(cases)("$name", async (entry) => {
    const actual = await stillwind(entry);
    if (entry.absent) {
      expect(actual.css).not.toContain(entry.absent);
    }
    const values = await measure(page, entry, actual);
    const expected = await tailwind(entry);
    if (entry.absent) {
      expect(expected.css).not.toContain(entry.absent);
    }
    expect(values).toEqual(await measure(page, entry, expected));
  });
});
