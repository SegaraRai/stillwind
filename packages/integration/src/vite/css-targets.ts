import type { BaseConfig } from "@stillwind/config";
import type { NativeCssMinifyOptions } from "@stillwind/native";
import type { ResolvedConfig } from "vite";

const browserTarget =
  /^(android|chrome|edge|firefox|ie|ios|opera|safari|samsung)(\d+(?:\.\d+){0,2})$/;

/**
 * Selects browser versions from Vite's resolved CSS target without inferring versions from JS levels.
 * @param target - Resolved CSS target, including disabled targets and language-level strings.
 * @returns A copy of the browser list, or an empty list if any entry cannot identify browser versions.
 */
export function inheritCssTarget(target: ResolvedConfig["build"]["cssTarget"]): string[] {
  const entries = typeof target === "string" ? [target] : target || [];
  return entries.every((entry) => browserTarget.test(entry)) ? [...entries] : [];
}

/**
 * Converts Stillwind browser versions to Lightning CSS's packed minimum versions.
 * @param target - Validated compiler targets. Omission and empty lists leave minifier targets unspecified.
 * @returns Browser versions with the oldest entry retained for each browser and ios named ios_saf.
 * @throws If a target cannot be represented by Lightning CSS's three one-byte version components.
 */
export function minifyTargets(target: BaseConfig["target"]): NativeCssMinifyOptions["targets"] {
  const entries = typeof target === "string" ? [target] : (target ?? []);
  if (!entries.length) {
    return undefined;
  }
  const targets: Record<string, number> = {};
  for (const entry of entries) {
    const match = browserTarget.exec(entry);
    const parts = match?.[2].split(".").map(Number);
    if (!match || !parts || parts.some((part) => part > 255)) {
      throw new Error(
        `Cannot represent CSS minifier target ${JSON.stringify(entry)}: each version component must be 0..255`,
      );
    }
    const [major, minor = 0, patch = 0] = parts;
    const version = major * 65536 + minor * 256 + patch;
    const browser = match[1] === "ios" ? "ios_saf" : match[1];
    targets[browser] = Math.min(targets[browser] ?? version, version);
  }
  return targets;
}
