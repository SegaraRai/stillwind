import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { readFileSync, writeFileSync, mkdirSync } from "node:fs";
import { stripTypeScriptTypes } from "node:module";
import { fileURLToPath } from "node:url";

// Run the repository's actual four CSS/native/upstream acceptance tests against
// both implementations without copying or modifying the tests on disk.
const testUrl = new URL("../../packages/host/test/css-source-map.test.mts", import.meta.url);
const results = [];
for (const variant of ["baseline", "candidate"]) {
  let source = readFileSync(testUrl, "utf8");
  source = source.replaceAll(
    '"../src/css-source-map.ts"',
    JSON.stringify(
      new URL(
        variant === "candidate"
          ? "./css-candidate.ts"
          : "../../packages/host/src/css-source-map.ts",
        import.meta.url,
      ).href,
    ),
  );
  source = source.replaceAll(
    '"../src/source-map.ts"',
    JSON.stringify(new URL("../../packages/host/src/source-map.ts", import.meta.url).href),
  );
  source = source.replace(
    /import \{([^}]+)\} from "(@[^"]+)";/g,
    (_, names, name) => `const {${names}} = require(${JSON.stringify(name)});`,
  );
  source =
    `import {createRequire} from 'node:module'; const require=createRequire(${JSON.stringify(testUrl.href)});\n` +
    source;
  const result = spawnSync(
    process.execPath,
    ["--input-type=module", "-e", stripTypeScriptTypes(source)],
    { encoding: "utf8", cwd: fileURLToPath(new URL("../..", import.meta.url)) },
  );
  console.log(variant + "\n" + result.stdout);
  assert.equal(result.status, 0, result.stderr);
  results.push({ variant, exitCode: result.status, output: result.stdout });
}
mkdirSync(new URL("./results/", import.meta.url), { recursive: true });
writeFileSync(
  new URL("./results/acceptance.json", import.meta.url),
  JSON.stringify({ date: new Date().toISOString(), results }, null, 2) + "\n",
);
