import { expect, test } from "vite-plus/test";

import { resolveServerSource } from "./server-source-resolve.ts";

const serverOnlyError = () =>
  new Error("Errored while resolving in `this.resolve`.", {
    cause: new Error("Server-only module referenced by client"),
  });

test("server source resolution retries React Router's server-only import error", async () => {
  const calls: unknown[] = [];
  const resolved = await resolveServerSource(
    async (specifier, importer, options) => {
      calls.push([specifier, importer, options]);
      if (calls.length < 3) {
        throw serverOnlyError();
      }
      return { id: "/app/server.server.js" };
    },
    "../server.server.js",
    "/app/routes/home.jsx",
  );

  expect(resolved).toEqual({ id: "/app/server.server.js" });
  expect(calls).toHaveLength(3);
  expect(calls[0]).toEqual([
    "../server.server.js",
    "/app/routes/home.jsx",
    { skipSelf: true, custom: { "react-router:dot-server": true } },
  ]);
});

test("server source resolution passes other resolver errors without a retry", async () => {
  let calls = 0;
  await expect(
    resolveServerSource(
      async () => {
        calls++;
        throw new Error("Cannot resolve");
      },
      "./missing.js",
      "/app/routes/home.jsx",
    ),
  ).rejects.toThrow("Cannot resolve");
  expect(calls).toBe(1);
});

test("server source resolution stops retrying after a bounded number of attempts", async () => {
  let calls = 0;
  const last = serverOnlyError();
  await expect(
    resolveServerSource(
      async () => {
        calls++;
        throw calls === 5 ? last : serverOnlyError();
      },
      "../server.server.js",
      "/app/routes/home.jsx",
    ),
  ).rejects.toBe(last);
  expect(calls).toBe(5);
});
