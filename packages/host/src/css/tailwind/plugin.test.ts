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
