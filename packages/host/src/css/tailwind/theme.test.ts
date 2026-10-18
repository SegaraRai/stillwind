import { expect, test } from "vite-plus/test";

import { keyPathToCssProperty, PluginTheme } from "./theme.ts";

const token = (name: string, value: string, fallback = false) => ({
  name,
  value,
  fallback,
  inline: false,
});

test("config key paths map to Tailwind's CSS theme variables", () => {
  expect(keyPathToCssProperty(["colors", "red", "500"])).toBe("color-red-500");
  expect(keyPathToCssProperty(["fontSize", "xs", "1", "lineHeight"])).toBe("text-xs--line-height");
  expect(keyPathToCssProperty(["transitionDuration", "DEFAULT"])).toBe(
    "default-transition-duration",
  );
  expect(keyPathToCssProperty(["spacing", "1.5"])).toBe("spacing-1_5");
  expect(keyPathToCssProperty(["backgroundImage", "hero"])).toBe("background-image-hero");
  expect(keyPathToCssProperty(["container", "center"])).toBeNull();
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
    ).css(),
  ).toContain("--color-*:initial;--color-only:red;");
  expect(new PluginTheme([{}], []).css()).toBe("");
});
