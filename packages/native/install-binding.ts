import { randomUUID } from "node:crypto";
import { mkdir, readFile, readdir, rename, rm, writeFile } from "node:fs/promises";
import { join } from "node:path";

const root = import.meta.dirname;
const staging = join(root, ".build/native");
await mkdir(join(root, "dist"), { recursive: true });

for (const name of await readdir(staging)) {
  if (name !== "index.js" && name !== "binding.d.ts" && !name.endsWith(".node")) {
    continue;
  }
  const destination = join(root, name === "binding.d.ts" ? name : `dist/${name}`);
  const content = await readFile(join(staging, name));
  try {
    if (content.equals(await readFile(destination))) {
      continue;
    }
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code !== "ENOENT") {
      throw error;
    }
  }

  const temporary = `${destination}.${randomUUID()}.tmp`;
  const retired = `${destination}.${randomUUID()}.retired.tmp`;
  await writeFile(temporary, content);
  try {
    try {
      await rename(temporary, destination);
    } catch (error) {
      if (!["EACCES", "EPERM"].includes((error as NodeJS.ErrnoException).code ?? "")) {
        throw error;
      }
      // Windows allows a loaded addon to be renamed, but prevents replacing its file.
      await rename(destination, retired);
      try {
        await rename(temporary, destination);
      } catch (error) {
        await rename(retired, destination);
        throw error;
      }
      try {
        await rm(retired);
      } catch (error) {
        if (!["EACCES", "EPERM", "EBUSY"].includes((error as NodeJS.ErrnoException).code ?? "")) {
          throw error;
        }
      }
    }
  } finally {
    await rm(temporary, { force: true });
  }
}
