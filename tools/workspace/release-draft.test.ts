import { readFileSync } from "node:fs";
import path from "node:path";

import { expect, test } from "vite-plus/test";

import {
  changelogAnchor,
  compareVersions,
  parseLedger,
  previousReleaseTag,
  releaseSections,
  renderDraft,
} from "./release-draft.ts";

const root = path.resolve(import.meta.dirname, "../..");

test("versions sort by semantic version precedence", () => {
  const versions = [
    "1.0.0",
    "0.10.0",
    "1.0.0-rc.1",
    "0.2.0",
    "1.0.0-alpha",
    "1.0.0-rc.0",
    "1.0.0-alpha.1",
  ];

  expect(versions.sort(compareVersions)).toEqual([
    "0.2.0",
    "0.10.0",
    "1.0.0-alpha",
    "1.0.0-alpha.1",
    "1.0.0-rc.0",
    "1.0.0-rc.1",
    "1.0.0",
  ]);
});

test("the previous release is the highest release tag below the version", () => {
  const tags = ["v0.1.0", "v0.3.0", "v0.5.0", "@stillwind/plugin-iconify@0.4.0", "v0.4.1"];

  expect(previousReleaseTag(tags, "0.4.1")).toBe("v0.3.0");
  expect(previousReleaseTag(tags, "0.1.0")).toBeUndefined();
});

test("ledger entries provide the package directory of each changelog section", () => {
  const ledger = [
    '"@stillwind/cli@0.4.0":',
    "  dir: packages/cli",
    "  intents:",
    "    - clear-pages-point",
    "stillwind@0.4.0:",
    "  dir: packages/core",
    "  intents:",
    "    - hidden-checks-stay",
    "",
  ].join("\n");

  expect(parseLedger(ledger)).toEqual([
    { name: "@stillwind/cli", version: "0.4.0", dir: "packages/cli" },
    { name: "stillwind", version: "0.4.0", dir: "packages/core" },
  ]);
  expect(
    parseLedger(readFileSync(path.join(root, ".changeset/ledger.yaml"), "utf8")),
  ).toContainEqual({
    name: "stillwind",
    version: "0.4.0",
    dir: "packages/core",
  });
});

test("a release includes the sections after the previous release, main package first", () => {
  const sections = [
    { name: "@stillwind/cli", version: "0.4.0", dir: "packages/cli" },
    { name: "stillwind", version: "0.3.0", dir: "packages/core" },
    { name: "stillwind", version: "0.4.0", dir: "packages/core" },
    { name: "stillwind", version: "0.4.1", dir: "packages/core" },
    { name: "stillwind", version: "0.5.0", dir: "packages/core" },
  ];

  expect(
    releaseSections(sections, "0.4.1", "0.3.0").map(({ name, version }) => `${name}@${version}`),
  ).toEqual(["stillwind@0.4.1", "stillwind@0.4.0", "@stillwind/cli@0.4.0"]);
  expect(releaseSections(sections, "0.4.1", undefined)).toHaveLength(4);
});

test("changelog anchors match GitHub heading fragments", () => {
  expect(changelogAnchor("0.4.1")).toBe("041");
  expect(changelogAnchor("1.0.0-rc.0")).toBe("100-rc0");
});

test("the draft has an agent request, the instructions in a comment, and the changelog links", () => {
  const draft = renderDraft({
    repository: "owner/name",
    commit: "abc123",
    version: "0.4.1",
    previousTag: "v0.3.0",
    sections: [{ name: "stillwind", version: "0.4.0", dir: "packages/core" }],
    prompt: readFileSync(path.join(import.meta.dirname, "release-notes-prompt.md"), "utf8"),
  });
  const link =
    "- [`stillwind` 0.4.0](https://github.com/owner/name/blob/abc123/packages/core/CHANGELOG.md#040)";
  const [request = "", rest = ""] = draft.split("<!--\n");
  const comment = rest.split("\n-->\n")[0];

  expect(request).toContain("`gh release view v0.4.1 --repo owner/name --json body --jq .body`");
  expect(request).toContain("`gh release edit v0.4.1 --repo owner/name --notes-file <file>`");
  expect(comment).toContain("Stillwind v0.4.1");
  expect(comment).toContain("Users upgrade from v0.3.0.");
  expect(comment).toContain(link);
  expect(comment).not.toMatch(/\{\{\w+\}\}/);
  expect(draft.slice(draft.indexOf("-->"))).toContain(`## Changelogs\n\n${link}\n`);
});

test("instructions that would break the HTML comment are rejected", () => {
  expect(() =>
    renderDraft({
      repository: "owner/name",
      commit: "abc123",
      version: "0.4.1",
      previousTag: undefined,
      sections: [],
      prompt: "Mention --if().",
    }),
  ).toThrow("must not contain `--`");
});
