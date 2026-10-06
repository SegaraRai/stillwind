import { relative } from "node:path";
import { pathToFileURL } from "node:url";

import { cleanModuleId, fileSourceKind, type ModuleDescriptor } from "./module-source.ts";

/**
 * Derives a compiler module identity relative to a filesystem root.
 *
 * @param root - Filesystem directory from which the identity is made relative.
 * @param id - Filesystem path to identify. Callers remove resolver queries before using this
 * helper.
 * @returns A relative path with forward slashes. Paths outside the root retain `..` segments. This
 * function does not enforce containment.
 */
export function compilerModuleId(root: string, id: string): string {
  return relative(root, id).replaceAll("\\", "/");
}

/**
 * Infers parser metadata from a filesystem path and assigns compiler and checker identities.
 *
 * @param root - Project directory used for relative compiler and projection identities.
 * @param id - Absolute source path, optionally followed by a query or fragment suffix.
 * @returns Metadata for the file after removing its query or fragment suffix.
 */
export function fileModuleDescriptor(root: string, id: string): ModuleDescriptor {
  const file = cleanModuleId(id);
  const { syntax, format } = fileSourceKind(file);
  return {
    moduleId: compilerModuleId(root, file),
    document: {
      uri: pathToFileURL(file).href,
      format,
    },
    checkerFileName: file,
    projectionFileName: compilerModuleId(root, file),
    syntax,
  };
}
