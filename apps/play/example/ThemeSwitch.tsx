import { style } from "stillwind/react";

import { toggle } from "./styles";

export type Theme = "system" | "light" | "dark";

const themes: [Theme, string][] = [
  ["system", "System"],
  ["light", "Light"],
  ["dark", "Dark"],
];

export function ThemeSwitch({
  value,
  onChange,
}: {
  value: Theme;
  onChange: (theme: Theme) => void;
}) {
  return (
    <div
      role="group"
      aria-label="Theme"
      {...style("flex p-1 gap-1 bg-surface-muted rounded-control")}
    >
      {themes.map(([theme, label]) => (
        <button
          key={theme}
          type="button"
          aria-pressed={theme === value}
          onClick={() => onChange(theme)}
          {...toggle}
        >
          <span
            aria-hidden="true"
            {...style(
              "icon-control",
              {
                system: "icon-lucide--monitor",
                light: "icon-lucide--sun",
                dark: "icon-lucide--moon",
              }[theme],
            )}
          />
          {/* The label appears once the header has room for it. */}
          <span {...style("sr-only sidebar:not-sr-only")}>{label}</span>
        </button>
      ))}
    </div>
  );
}
