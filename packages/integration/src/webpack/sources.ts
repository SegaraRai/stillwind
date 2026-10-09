import { createHash } from "node:crypto";
import path from "node:path";

import { cleanModuleId, fileModuleDescriptor } from "@stillwind/host";

/**
 * Keeps each query-selected derivative's compiler, checker, and source-map identities distinct.
 * @param root - Absolute project root used to derive compiler identities.
 * @param id - Full source identity, including any query or fragment selecting transformed source.
 * @returns Ordinary file metadata without a suffix, or distinct module/document identities and a hashed checker filename.
 */
export function sourceDescriptor(root: string, id: string) {
  const descriptor = fileModuleDescriptor(root, id);
  const file = cleanModuleId(id);
  const suffix = id.slice(file.length);
  if (!suffix) {
    return descriptor;
  }
  const extension = path.extname(file);
  const key = createHash("sha256").update(id).digest("hex");
  const checkerFileName = path.join(
    path.dirname(file),
    `.${path.basename(file, extension)}.stillwind-${key}${extension}`,
  );
  return {
    ...descriptor,
    moduleId: descriptor.moduleId + suffix,
    document: {
      ...descriptor.document,
      uri: descriptor.document.uri + suffix,
    },
    checkerFileName,
    projectionFileName: checkerFileName.replaceAll("\\", "/"),
  };
}
