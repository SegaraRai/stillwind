import { expect, test } from "vite-plus/test";

import { installLibraries } from "./libraries.ts";

test("virtual library installation supplies absent packages and preserves installed versions", () => {
  const files = new Map([["/existing/node_modules/stillwind/package.json", "workspace version"]]);
  installLibraries(
    files,
    [
      ["/node_modules/typescript/lib/lib.d.ts", "lib"],
      ["/__stillwind/packages/stillwind/package.json", "bundled version"],
      ["/__stillwind/packages/stillwind/index.css", "bundled CSS"],
    ],
    ["/empty", "/existing"],
  );
  expect(files.get("/empty/node_modules/stillwind/index.css")).toBe("bundled CSS");
  expect(files.get("/existing/node_modules/stillwind/package.json")).toBe("workspace version");
  expect(files.has("/existing/node_modules/stillwind/index.css")).toBe(false);
  expect(files.get("/node_modules/typescript/lib/lib.d.ts")).toBe("lib");
});
