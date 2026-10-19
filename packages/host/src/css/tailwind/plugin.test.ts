import plugin from "tailwindcss/plugin";
import { expect, test } from "vite-plus/test";

import type { TailwindPlugin } from "../plugins.ts";
import { configurePlugin, TailwindRuntime } from "./plugin.ts";

/** Creates a runtime for configured handlers, one for each `@plugin` statement. */
function runtime(...plugins: unknown[]) {
  return new TailwindRuntime(
    plugins.map((entry) => configurePlugin(entry as TailwindPlugin, "")),
    [],
  );
}

test("registrations become ordinary utilities and functional definitions of their statement", () => {
  const first = plugin((api) => {
    api.addUtilities({
      ".tidy": {
        margin: "0",
      },
    });
    api.matchUtilities(
      {
        probe: (value) => ({
          width: value,
        }),
      },
      {
        values: {
          4: "4px",
        },
      },
    );
  });
  const second = plugin((api) => api.addVariant("hocus", "&:hover"));
  const tailwind = runtime(first, second);

  expect(tailwind.resolve(["probe-4", "-probe-4", "probe-x"])).toBe(true);
  expect(tailwind.css(0)).toBe(
    [
      "@utility tidy{margin:0;}",
      "@stillwind-plugin-family utility probe;",
      "@utility probe-4 (functional){width:4px;}",
    ].join("\n"),
  );
  expect(tailwind.css(1)).toBe("@custom-variant hocus{&:hover{@slot;}}");
  expect(tailwind.resolve(["probe-4"])).toBe(false);
});

test("dynamic registrations pass the selectors of their named values as marker sources", () => {
  const tailwind = runtime(
    plugin((api) => {
      api.matchUtilities(
        {
          stack: (value) => ({
            [`.stack-root &`]: {
              gap: value,
            },
          }),
        },
        {
          values: {
            sm: "4px",
          },
        },
      );
      api.matchVariant("theme", (value) => `.theme-${String(value)} &`, {
        values: {
          dark: "dark",
        },
      });
    }),
  );

  expect(tailwind.css(0)).toBe(
    [
      "@stillwind-plugin-family utility stack;",
      '@stillwind-marker-source ".stack-root &{}";',
      "@stillwind-plugin-family variant theme;",
      '@stillwind-marker-source ".theme-dark &{@slot;}";',
    ].join("\n"),
  );
});

test("typed registrations stop on invalid arbitrary modifiers, and any-typed ones are fallbacks", () => {
  const tailwind = runtime(
    plugin((api) => {
      api.matchUtilities(
        {
          tone: (value) => ({
            color: value,
          }),
        },
        {
          type: "color",
          values: {
            brand: "red",
          },
        },
      );
      api.matchUtilities(
        {
          line: (value) => ({
            "border-width": value,
          }),
        },
        {
          type: ["length", "any"],
          values: {
            thin: "1px",
          },
        },
      );
    }),
  );
  tailwind.resolve(["tone-[red]/nope", "tone-brand/nope", "line-thin"]);

  const css = tailwind.css(0);
  expect(css).toContain("@utility tone-\\[red\\]\\/nope (functional, stop){}");
  expect(css).not.toContain("tone-brand");
  expect(css).toContain("@utility line-thin (functional, fallback){border-width:1px;}");
});

/**
 * Records the arguments that a `matchUtilities` callback receives for each candidate.
 * @param options - Options of the registration.
 * @param candidates - Utility keys to resolve.
 * @returns The callback arguments in call order.
 */
function callbackArguments(options: object, candidates: string[]) {
  const calls: {
    value: unknown;
    modifier: string | null;
  }[] = [];
  const tailwind = runtime(
    plugin((api) =>
      api.matchUtilities(
        {
          probe: (value, { modifier }) => {
            calls.push({
              value,
              modifier,
            });
            return {
              width: String(value),
            };
          },
        },
        options,
      ),
    ),
  );
  // Registration runs the named values to find marker classes; only requests count here.
  calls.length = 0;
  tailwind.resolve(candidates);
  return calls;
}

test("arbitrary values and modifiers reach callbacks decoded as Tailwind decodes them", () => {
  expect(
    callbackArguments(
      {
        values: {
          one: "1px",
        },
        modifiers: "any",
      },
      [
        "probe-[var(--my_width)]",
        "probe-[calc(1px+2px)]",
        "probe-['a_b']",
        "probe-one/[foo_bar]",
        String.raw`probe-one/[foo\_bar]`,
        "probe-one/(--alpha)",
      ],
    ),
  ).toEqual([
    { value: "var(--my_width)", modifier: null },
    { value: "calc(1px + 2px)", modifier: null },
    { value: "'a b'", modifier: null },
    { value: "1px", modifier: "foo bar" },
    { value: "1px", modifier: "foo_bar" },
    { value: "1px", modifier: "var(--alpha)" },
  ]);
});

test("type hints and Tailwind's type inference select typed registrations", () => {
  expect(
    callbackArguments({ type: "length" }, [
      "probe-(length:--my-size)",
      "probe-[length:var(--my-size)]",
      "probe-[calc(100%_-_1rem)]",
      "probe-[calc(1+2)]",
      "probe-(--my-size)",
      "probe-(color:--my-size)",
      "probe-[red]",
    ]),
  ).toEqual([
    { value: "var(--my-size)", modifier: null },
    { value: "var(--my-size)", modifier: null },
    { value: "calc(100% - 1rem)", modifier: null },
    { value: "calc(1 + 2)", modifier: null },
  ]);
});

test("named variant values must be strings, and the bare root uses DEFAULT", () => {
  const tailwind = runtime(
    plugin((api) =>
      api.matchVariant("state", (value) => `&[data-state="${String(value)}"]`, {
        values: {
          DEFAULT: "on",
          one: 1 as unknown as string,
          open: "open",
        },
      }),
    ),
  );
  tailwind.resolve([
    "@variant state",
    "@variant state-one",
    "@variant state-open",
    "@variant state-[a_b]",
  ]);

  const css = tailwind.css(0);
  expect(css).toContain('@custom-variant state{&[data-state="on"]{@slot;}}');
  expect(css).toContain('@custom-variant state-open{&[data-state="open"]{@slot;}}');
  expect(css).toContain(String.raw`@custom-variant state-\[a_b\]{&[data-state="a b"]{@slot;}}`);
  expect(css).not.toContain("state-one");
});

test("config() reads Tailwind's resolved keys with bracket paths, and the last config sets them", () => {
  const observed: unknown[] = [];
  const tailwind = runtime(
    plugin(() => {}, {
      important: true,
      prefix: "first",
    }),
    plugin(
      (api) => {
        observed.push(
          api.config("theme[colors][brand]"),
          api.config("theme.colors.brand"),
          api.config("custom[one]", "missing"),
          api.config("important"),
          api.config("prefix"),
        );
      },
      {
        important: "#app",
        theme: {
          colors: {
            brand: "red",
          },
        },
        custom: { one: "found" },
      } as never,
    ),
  );
  expect(observed).toEqual(["red", "red", "missing", "#app", "first"]);
  // The prefix is not applied to candidates.
  expect(tailwind.css(0)).toBe(
    "@stillwind-plugin-theme{--color-*:initial;--color-brand:red;}\n@stillwind-plugin-important #app;",
  );
  expect(
    runtime(
      plugin(() => {}, {
        important: true,
      }),
    ).css(0),
  ).toBe("@stillwind-plugin-important;");
  expect(
    runtime(
      plugin(() => {}, {
        important: false,
      }),
    ).css(0),
  ).toBe("");
});
