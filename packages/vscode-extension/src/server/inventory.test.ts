import { expect, test } from "vite-plus/test";

import { consumeWorkspaceFiles } from "./inventory.ts";

test("workspace capture releases its file list while the inventory keeps decoded paths and text", () => {
  const initializationOptions = {
    files: [["file:///workspace/source%20file.ts", "initial source"]] as [string, string][],
    configFile: "/workspace/stillwind.config.ts",
    completion: {
      registry: false,
    },
  };
  const files = consumeWorkspaceFiles(initializationOptions);
  expect(initializationOptions).toEqual({
    files: [],
    configFile: "/workspace/stillwind.config.ts",
    completion: {
      registry: false,
    },
  });
  expect(files).toEqual(new Map([["/workspace/source file.ts", "initial source"]]));

  const recaptured = {
    files: [["file:///workspace/source%20file.ts", "edited source"]] as [string, string][],
  };
  expect(consumeWorkspaceFiles(recaptured)).toEqual(
    new Map([["/workspace/source file.ts", "edited source"]]),
  );
  expect(files.get("/workspace/source file.ts")).toBe("initial source");
});
