import { createHash } from "node:crypto";
import path from "node:path";

import { cleanModuleId, fileModuleDescriptor } from "@stillwind/host";

/** Keeps each derivative's compiler, checker and source-map identities distinct. */
export function sourceDescriptor(root: string, id: string) {
  const descriptor = fileModuleDescriptor(root, id);
  const file = cleanModuleId(id);
  const suffix = id.slice(file.length);
  if (!suffix) {
    return descriptor;
  }
  const extension = path.extname(file);
  const key = createHash("sha256").update(id).digest("hex");
  return {
    ...descriptor,
    moduleId: descriptor.moduleId + suffix,
    document: { ...descriptor.document, uri: descriptor.document.uri + suffix },
    checkerFileName: path.join(root, ".stillwind-checker", key + extension),
    projectionFileName: descriptor.projectionFileName + suffix,
  };
}
