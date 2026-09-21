import { isAbsolute } from "node:path";
import { fileURLToPath } from "node:url";

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

export function resolvedFile(
  result: { id: string; external?: boolean | string } | null,
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

export function resolveResources(css: string, references = new Map<string, string>()): string {
  for (const [placeholder, url] of references) {
    css = css.replaceAll(placeholder, url);
  }
  return css;
}
