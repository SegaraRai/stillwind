import { useEffect, useState } from "react";

/**
 * Color theme that the user chose. `system` follows the operating system's color scheme.
 */
export type ColorThemePreference = "system" | "light" | "dark";

/** Local storage key that the website and the playground share. */
const STORAGE_KEY = "stillwind:theme";
const PREFERENCES: ColorThemePreference[] = ["system", "light", "dark"];

function normalize(value: string | null | undefined): ColorThemePreference {
  return value === "light" || value === "dark" ? value : "system";
}

/**
 * Tracks the color theme preference and the color scheme it resolves to.
 * The preference is stored in `data-theme` on the root element and in local storage, so the
 * stylesheet and the website use the same value. Changes in another tab are applied here.
 *
 * @returns The preference, the resolved color scheme, and a function that selects the next
 *   preference in the order system, light, dark.
 */
export function useColorTheme(): {
  preference: ColorThemePreference;
  scheme: "light" | "dark";
  cycle: () => void;
} {
  const root = document.documentElement;
  const [preference, setPreference] = useState(() => normalize(root.dataset.theme));
  const [systemDark, setSystemDark] = useState(
    () => matchMedia("(prefers-color-scheme: dark)").matches,
  );

  useEffect(() => {
    const media = matchMedia("(prefers-color-scheme: dark)");
    const onMedia = () => setSystemDark(media.matches);
    const onStorage = (event: StorageEvent) => {
      if (event.key === STORAGE_KEY || event.key === null) {
        setPreference(normalize(event.newValue));
      }
    };
    media.addEventListener("change", onMedia);
    window.addEventListener("storage", onStorage);
    return () => {
      media.removeEventListener("change", onMedia);
      window.removeEventListener("storage", onStorage);
    };
  }, []);

  useEffect(() => {
    if (preference === "system") {
      delete root.dataset.theme;
    } else {
      root.dataset.theme = preference;
    }
  }, [preference]);

  const cycle = () => {
    const next = PREFERENCES[(PREFERENCES.indexOf(preference) + 1) % PREFERENCES.length]!;
    setPreference(next);
    try {
      if (next === "system") {
        localStorage.removeItem(STORAGE_KEY);
      } else {
        localStorage.setItem(STORAGE_KEY, next);
      }
    } catch {}
  };

  const scheme = preference === "system" ? (systemDark ? "dark" : "light") : preference;
  return { preference, scheme, cycle };
}
