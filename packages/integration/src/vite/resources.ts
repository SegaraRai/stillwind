import { isAbsolute } from "node:path";
import { fileURLToPath } from "node:url";

/** Resolved local resource retained for CSS emission and development access checks. */
export interface StylesheetResource {
  /** Resolver identity, including any resource query. */
  id: string;
  /** Canonical absolute path checked before reading or serving the resource. */
  file: string;
}

/**
 * Converts local resource placeholders to escaped Vite filesystem URLs.
 * @param resources - Placeholder records whose files have already passed access checks.
 * @returns A new map of placeholders to JSON-escaped URL contents without surrounding quotes.
 */
export function resourceReplacements(
  resources: ReadonlyMap<string, StylesheetResource>,
): Map<string, string> {
  return new Map(
    [...resources].map(([placeholder, resource]) => [
      placeholder,
      JSON.stringify(resourceUrl(resource.file)).slice(1, -1),
    ]),
  );
}

/**
 * Encodes an absolute file path as a Vite filesystem URL.
 * @param file - Canonical absolute path, using either path separator spelling.
 * @returns An /@fs/ URL with each path segment encoded for use in stylesheet resource requests.
 */
export function resourceUrl(file: string): string {
  return `/@fs/${file
    .replaceAll("\\", "/")
    .split("/")
    .map((segment) =>
      encodeURIComponent(segment)
        .replaceAll("'", "%27")
        .replaceAll("(", "%28")
        .replaceAll(")", "%29"),
    )
    .join("/")}`;
}

/**
 * Extracts a readable identity from a bundler resolution without checking file existence or access.
 * @param result - Resolution, possibly external or a file URI. Absolute external paths remain readable.
 * @param description - Resource description included in failure messages.
 * @returns The resolved identity, converting file URIs to filesystem paths.
 * @throws If resolution is absent, an external identity is not an absolute path, or a file URI is invalid.
 */
export function resolvedFile(
  result: {
    /** Resolved identity or file URI returned by the bundler. */
    id: string;
    /** Truthy values mark external modules, which require a file URI or absolute path. */
    external?: boolean | string;
  } | null,
  description: string,
): string {
  if (!result) {
    throw new Error(`Cannot resolve ${description}`);
  }
  if (result.id.startsWith("file:")) {
    return fileURLToPath(result.id);
  }
  if (!result.external || isAbsolute(result.id)) {
    return result.id;
  }
  throw new Error(`Cannot read external ${description}: ${result.id}`);
}

/**
 * Replaces recorded CSS tokens in map order without further URL escaping.
 * @param css - Generated text containing resource placeholders.
 * @param references - Placeholders mapped to replacement contents already escaped for CSS.
 * @returns Replaced CSS. The supplied map is unchanged.
 */
export function resolveResources(css: string, references = new Map<string, string>()): string {
  for (const [placeholder, url] of references) {
    css = css.replaceAll(placeholder, url);
  }
  return css;
}
