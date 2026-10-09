import { writeFile } from "node:fs/promises";
import path from "node:path";

import { expect, test } from "vite-plus/test";

import { compileContext, createTestServer, redStyleSource } from "./vite-server-fixture.ts";

test("Vite warns once for naming fallback shared by reference and optimized renders", async () => {
  const { project, root, server } = await createTestServer(
    {
      naming: {
        layerName: () => {
          throw new Error("layer rejected");
        },
      },
    },
    "@utility red { color:red; } @layer { @stillwind-slot utilities; }",
  );
  const entry = path.join(root, "entry.tsx");
  await writeFile(entry, redStyleSource);
  const warnings: string[] = [];
  const context = {
    ...compileContext(server, "client"),
    warn: (message: string) => warnings.push(message),
  };
  expect(await project.compile("client", redStyleSource, entry, context)).toBeDefined();
  const plan = await project.documentPlan("client", context, true);
  expect(plan.css).toContain("color:red");
  expect(plan.rejectedNames).toHaveLength(1);
  expect(warnings).toHaveLength(1);
  expect(warnings[0]).toContain("layer rejected");
  expect(warnings[0]).toContain(plan.rejectedNames[0].hash);
});
