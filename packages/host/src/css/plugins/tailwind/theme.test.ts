import { expect, test } from "vite-plus/test";

import { builtinLegacyThemeKeys } from "./legacy-keys.testing.ts";
import { keyPathToCssProperty, PluginTheme } from "./theme.ts";

const legacyKeys = builtinLegacyThemeKeys();
const namespaces = Object.fromEntries(legacyKeys.map(({ key, namespace }) => [key, namespace]));

const token = (name: string, value: string, fallback = false) => ({
  name,
  value,
  fallback,
  inline: false,
});

test("config key paths map to Tailwind's CSS theme variables", () => {
  expect(keyPathToCssProperty(["colors", "red", "500"], namespaces)).toBe("color-red-500");
  expect(keyPathToCssProperty(["fontSize", "xs", "1", "lineHeight"], namespaces)).toBe(
    "text-xs--line-height",
  );
  expect(keyPathToCssProperty(["transitionDuration", "DEFAULT"], namespaces)).toBe(
    "default-transition-duration",
  );
  expect(keyPathToCssProperty(["spacing", "1.5"], namespaces)).toBe("spacing-1_5");
  expect(keyPathToCssProperty(["backgroundImage", "hero"], namespaces)).toBe(
    "background-image-hero",
  );
  expect(keyPathToCssProperty(["container", "center"], namespaces)).toBeNull();
});

test("theme() prefers ordinary CSS, then plugin config, then CSS defaults and v3 defaults", () => {
  const theme = new PluginTheme(
    [
      {
        theme: {
          extend: {
            colors: {
              brand: "#000",
              neon: "#0f0",
            },
            opacity: {
              ghost: "0.1",
            },
          },
        },
      },
    ],
    [token("--color-brand", "#123456"), token("--color-neon", "#111", true)],
    legacyKeys,
  );
  expect(theme.get("colors.brand")).toBe("#123456");
  expect(theme.get("colors.neon")).toBe("#0f0");
  expect(theme.get("transitionDuration.150")).toBe("150ms");
  expect(theme.get("colors.brand / 50%")).toBe("color-mix(in oklab, #123456 50%, transparent)");
  expect(theme.get("colors.missing", "none")).toBe("none");
});

test("only the plugins' own configuration is written back as CSS", () => {
  const css = new PluginTheme(
    [
      {
        theme: {
          extend: {
            colors: { neon: "#0f0" },
            opacity: {
              ghost: "0.1",
            },
          },
        },
      },
    ],
    [],
    legacyKeys,
  ).css();
  expect(css).toContain("--color-neon:#0f0;");
  expect(css).toContain("--opacity-ghost:10%;");
  // Defaults that the plugins did not configure stay out of the CSS theme.
  expect(css).not.toContain("--transition-duration");
  expect(
    new PluginTheme(
      [
        {
          theme: {
            colors: {
              only: "red",
            },
          },
        },
      ],
      [],
      legacyKeys,
    ).css(),
  ).toContain("--color-*:initial;--color-only:red;");
  expect(new PluginTheme([{}], [], legacyKeys).css()).toBe("");
});

test("namespace roots retain DEFAULT while leaf lookups return the value", () => {
  const shadow = "0 1px 2px #000";
  for (const extra of [[], [token("--shadow-soft", "0 2px 4px #123")]]) {
    const theme = new PluginTheme(
      [],
      [
        token("--shadow", shadow),
        ...extra,
        token("--color-brand", "red"),
        token("--color-brand-deep", "maroon"),
      ],
      legacyKeys,
    );
    for (const root of ["boxShadow", "shadow"]) {
      expect(theme.get(root)).toEqual(
        expect.objectContaining({
          DEFAULT: shadow,
        }),
      );
      expect(theme.get(`${root}.DEFAULT`)).toBe(shadow);
    }
    expect(theme.get("colors.brand")).toBe("red");
    expect(theme.get("colors.brand.DEFAULT")).toBe("red");
    expect(theme.get("--shadow")).toBe(shadow);
  }
});

test("namespace DEFAULT values keep CSS and config precedence", () => {
  for (const fallback of [false, true]) {
    const theme = new PluginTheme(
      [
        {
          theme: {
            extend: {
              boxShadow: {
                DEFAULT: "0 4px 8px blue",
                custom: "0 8px 16px green",
              },
            },
          },
        },
      ],
      [token("--shadow", "0 1px 2px red", fallback)],
      legacyKeys,
    );
    const expected = fallback ? "0 4px 8px blue" : "0 1px 2px red";
    expect(theme.get("boxShadow")).toEqual(
      expect.objectContaining({
        DEFAULT: expected,
        custom: "0 8px 16px green",
      }),
    );
    expect(theme.get("boxShadow.DEFAULT")).toBe(expected);
  }
});

test("values that refer to their own variable are inlined instead of declared", () => {
  expect(
    new PluginTheme(
      [
        {
          theme: {
            extend: {
              colors: {
                primary: "var(--color-primary)",
                accent: "var(--color-accent, red)",
                brand: "var(--color-primary)",
              },
            },
          },
        },
      ],
      [],
      legacyKeys,
    ).css(),
  ).toBe(
    "@stillwind-plugin-theme{--color-brand:var(--color-primary);}" +
      "@stillwind-plugin-theme inline{--color-primary:var(--color-primary);--color-accent:var(--color-accent, red);}",
  );
});

test("fontFamily.sans and fontFamily.mono set the preflight fonts unless CSS defines the family", () => {
  const css = (tokens: ReturnType<typeof token>[]) =>
    new PluginTheme(
      [
        {
          theme: {
            extend: {
              fontFamily: {
                sans: [
                  "Inter, sans-serif",
                  {
                    fontFeatureSettings: '"cv11"',
                  },
                ],
                mono: ["Fira Code", "monospace"],
              },
            },
          },
        },
      ],
      tokens,
      legacyKeys,
    ).css();
  expect(css([token("--font-sans", "Arial", true)])).toContain(
    '--default-font-family:Inter, sans-serif;--default-font-feature-settings:"cv11";--default-font-variation-settings:normal;' +
      "--default-mono-font-family:Fira Code, monospace;--default-mono-font-feature-settings:normal;--default-mono-font-variation-settings:normal;",
  );
  const ordinary = css([token("--font-sans", "Georgia")]);
  expect(ordinary).not.toContain("--default-font-family");
  expect(ordinary).toContain("--default-mono-font-family:Fira Code, monospace;");
  expect(
    new PluginTheme(
      [
        {
          theme: {
            fontFamily: {
              serif: "Georgia",
            },
          },
        },
      ],
      [],
      legacyKeys,
    ).css(),
  ).not.toContain("--default-");
});
