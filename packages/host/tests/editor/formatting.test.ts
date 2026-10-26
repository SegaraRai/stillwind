import { lintPresets } from "@stillwind/config";
import { expect, test } from "vite-plus/test";

import { applyEdits } from "../../src/editor/sort.ts";
import { project, theme } from "./support.ts";

test("lint false disables reports and fixes while explicit sorting remains available", async () => {
  const source = 'import {style} from "stillwind/react"; const value=style("p-[16px] flex block");';
  const current = project(source, {
    lint: false,
  });
  try {
    const graph = await current.analyze("/workspace/entry.tsx");
    expect(graph.batches.flatMap((batch) => batch.reports)).toEqual([]);
    expect(
      (await current.query("/workspace/entry.tsx", {
        kind: "fixes",
      }))!.edits,
    ).toEqual([]);
    const sorted = applyEdits(
      source,
      (await current.query("/workspace/entry.tsx", {
        kind: "sort",
      }))!.edits!,
    );
    expect(sorted).not.toBe(source);
    expect(sorted).toContain("p-[16px]");
  } finally {
    await current.close();
  }
});

test("sort false disables explicit and lint sorting while preserving other lint fixes", async () => {
  const source =
    'import {style} from "stillwind/react"; const base=style("p-4"); const ref=style.ref(); const value=style([base,ref,"p-[16px] flex block",{"--z":value}]);';
  const current = project(source, {
    sort: false,
    lint: {
      extends: [lintPresets.recommended, lintPresets.stylistic],
    },
  });
  try {
    const graph = await current.analyze("/workspace/entry.tsx");
    expect(
      graph.batches
        .flatMap((batch) => batch.reports)
        .some((report) => report.code === "prefer-sorted-utilities"),
    ).toBe(false);
    expect(
      (await current.query("/workspace/entry.tsx", {
        kind: "sort",
      }))!.edits,
    ).toEqual([]);
    const fixed = applyEdits(
      source,
      (await current.query("/workspace/entry.tsx", {
        kind: "fixes",
      }))!.edits!,
    );
    expect(fixed).toContain('style([base,ref,"p-4 flex",{"--z":value}])');
  } finally {
    await current.close();
  }
});

test("canonical lint uses native policy, suppression, exact escaped ranges, and separate sorting actions", async () => {
  const source = String.raw`import {style} from "stillwind/react";
// 日本語 🦀
const card = style("p-[16px] flex block border-[2px]");
// swlint-disable-next-line prefer-canonical-utilities
const ignored = style("p-[16px]");
const escaped = style("p-\u005b16px\u005d");`;
  const current = project(source);
  try {
    const graph = await current.analyze("/workspace/entry.tsx");
    const reports = graph.batches.flatMap((batch) => batch.reports);
    expect(reports.filter((report) => report.code === "prefer-canonical-utilities")).toHaveLength(
      2,
    );
    expect(reports.some((report) => report.code === "prefer-sorted-utilities")).toBe(false);
    const fixes = (await current.query("/workspace/entry.tsx", {
      kind: "fixes",
    }))!.edits!;
    const fixed = applyEdits(source, fixes);
    expect(fixed).toContain('style("p-4 flex border-2")');
    expect(fixed).toContain('const ignored = style("p-[16px]")');
    expect(fixed).toContain('const escaped = style("p-4")');
    const sorted = applyEdits(
      source,
      (await current.query("/workspace/entry.tsx", {
        kind: "sort",
      }))!.edits!,
    );
    expect(sorted).not.toBe(source);
    expect(sorted).toContain("p-[16px]");
    const next = project(fixed);
    try {
      expect(
        (await next.query("/workspace/entry.tsx", {
          kind: "fixes",
        }))!.edits,
      ).toEqual([]);
    } finally {
      await next.close();
    }
  } finally {
    await current.close();
  }
});

test("project severity supports explicit conversion options and explicit sorting ignores disabled lint", async () => {
  const source =
    'import {style} from "stillwind/react"; const card = style("p-[16px] flex block");';
  const current = project(
    source,
    {
      lint: {
        extends: [lintPresets.recommended],
        rules: {
          "prefer-canonical-utilities": {
            pxToRem: false,
            severity: "deny",
          },
          "prefer-sorted-utilities": "off",
        },
      },
    },
    theme,
  );
  try {
    const graph = await current.analyze("/workspace/entry.tsx");
    expect(graph.batches.flatMap((batch) => batch.reports).map((report) => report.code)).toEqual([
      "prefer-canonical-utilities",
      "conflicting-utilities",
    ]);
    expect(
      (await current.query("/workspace/entry.tsx", {
        kind: "sort",
      }))!.edits?.length,
    ).toBeGreaterThan(0);
    const fixed = applyEdits(
      source,
      (await current.query("/workspace/entry.tsx", { kind: "fixes" }))!.edits!,
    );
    expect(fixed).toContain('style("p-[16px] flex")');
  } finally {
    await current.close();
  }
});

test("arrays keep dynamic boundaries, category order, and comments, while fix all composes nonoverlapping edits", async () => {
  const source = `import {style} from "stillwind/react";
const first = style("block"); const second = style("flex"); const marker = style.ref();
const value = style([
  "p-[16px]", // trailing comment
  first, second, marker,
  unknown,
  "flex", "block",
]);`;
  const current = project(source, {
    lint: {
      extends: [lintPresets.recommended],
      rules: {
        "prefer-sorted-utilities": "warn",
      },
    },
  });
  try {
    const edits = (await current.query("/workspace/entry.tsx", {
      kind: "fixes",
    }))!.edits!;
    expect(edits.every((edit, index) => index === 0 || edits[index - 1].end <= edit.start)).toBe(
      true,
    );
    const fixed = applyEdits(source, edits);
    expect(fixed).toContain("// trailing comment");
    const array = fixed.slice(fixed.indexOf("const value"));
    expect(array.indexOf("first")).toBeLessThan(array.indexOf("second"));
    expect(fixed).toMatch(/"flex",\s*"block"/);
  } finally {
    await current.close();
  }
});

test("sort targets are independent and base inputs retain their original order", async () => {
  const source =
    'import {style} from "stillwind/react"; const z=style("p-4"); const a=style("block"); const marker=style.ref(); const value=style([z,a,marker,"flex block"]);';
  const current = project(source, {
    sort: {
      stringLiterals: false,
    },
  });
  try {
    const sorted = applyEdits(
      source,
      (await current.query("/workspace/entry.tsx", {
        kind: "sort",
      }))!.edits!,
    );
    expect(sorted).toContain('[marker,z,a,"flex block"]');
    const next = project(sorted, {
      sort: {
        stringLiterals: false,
      },
    });
    try {
      expect(
        (await next.query("/workspace/entry.tsx", {
          kind: "sort",
        }))!.edits,
      ).toEqual([]);
    } finally {
      await next.close();
    }
  } finally {
    await current.close();
  }
  const strings = project(source, {
    sort: {
      arrays: false,
    },
  });
  try {
    const sorted = applyEdits(
      source,
      (await strings.query("/workspace/entry.tsx", {
        kind: "sort",
      }))!.edits!,
    );
    expect(sorted).toContain('[z,a,marker,"block flex"]');
  } finally {
    await strings.close();
  }
});

test("array sorting retains comments attached after an expression", async () => {
  const source =
    'import {style} from "stillwind/react"; const value=style(["flex" /* attached */, "block", "p-4"]);';
  const current = project(source);
  try {
    const sorted = applyEdits(
      source,
      (await current.query("/workspace/entry.tsx", {
        kind: "sort",
      }))!.edits!,
    );
    expect(sorted).toContain('"flex" /* attached */');
    expect(sorted.match(/attached/g)).toHaveLength(1);
  } finally {
    await current.close();
  }
});

test("input groups retain category order and place each ref marker before its projections", async () => {
  const source = `import {style} from "stillwind/react";
// 日本語 🦀
const z=style({"hover:":"p-4"}); const a=style(["p-8", {"--x":1}]);
const first=style.ref(); const alias=first; const second=style.ref();
const value=style([{ "--z":1 }, {"--z":2}, "flex block", z, second.asContainer(),
 first.positionAnchor(), a, alias, second, first.asAnchor(), "p-4"]);`;
  const current = project(source);
  try {
    const result = applyEdits(
      source,
      (await current.query("/workspace/entry.tsx", {
        kind: "sort",
      }))!.edits!,
    );
    expect(result.replace(/\s+/g, " ")).toContain(
      'style([ second, second.asContainer(), alias, first.positionAnchor(), first.asAnchor(), z, a, "block flex", "p-4",{ "--z":1 }, {"--z":2}])',
    );
    const next = project(result);
    try {
      expect(
        (await next.query("/workspace/entry.tsx", {
          kind: "sort",
        }))!.edits,
      ).toEqual([]);
    } finally {
      await next.close();
    }
  } finally {
    await current.close();
  }
});

test("argument grouping preserves base and utility order regardless of their generated CSS", async () => {
  const source = `import {style} from "stillwind/react"; const z=style("p-4"); const a=style("p-8"); const ref=style.ref(); const value=style("flex", z, "block", a, ref);`;
  const current = project(source);
  try {
    expect(
      applyEdits(
        source,
        (await current.query("/workspace/entry.tsx", {
          kind: "sort",
        }))!.edits!,
      ),
    ).toContain('style( ref, z, a,"flex", "block")');
  } finally {
    await current.close();
  }
});

test("suppression keeps grouping fixes inside approved ranges while explicit sorting remains available", async () => {
  const source = `import {style} from "stillwind/react"; const base=style("block"); const ref=style.ref();
// swlint-disable-next-line prefer-sorted-utilities
const ignored=style(["flex",base,ref]);
const active=style(["flex",base,ref]);`;
  const current = project(source, {
    lint: {
      extends: [lintPresets.recommended],
      rules: {
        "prefer-sorted-utilities": "warn",
      },
    },
  });
  try {
    const fixes = applyEdits(
      source,
      (await current.query("/workspace/entry.tsx", {
        kind: "fixes",
      }))!.edits!,
    );
    expect(fixes).toContain('const ignored=style(["flex",base,ref])');
    expect(fixes).toContain('const active=style([ref,base,"flex"])');
    const sorted = applyEdits(
      source,
      (await current.query("/workspace/entry.tsx", {
        kind: "sort",
      }))!.edits!,
    );
    expect(sorted).toContain('const ignored=style([ref,base,"flex"])');
  } finally {
    await current.close();
  }
});

test("sorting suppression reads directives after JSX text with an apostrophe", async () => {
  const source = `import {style} from "stillwind/react"; const base=style("block"); const ref=style.ref();
const note=<p>Don't</p>;
// swlint-disable-next-line prefer-sorted-utilities
const ignored=style(["flex",base,ref]);`;
  const current = project(source, {
    lint: {
      extends: [lintPresets.recommended],
      rules: {
        "prefer-sorted-utilities": "warn",
      },
    },
  });
  try {
    const graph = await current.analyze("/workspace/entry.tsx");
    expect(
      graph.batches
        .flatMap((batch) => batch.reports)
        .some((report) => report.code === "prefer-sorted-utilities"),
    ).toBe(false);
    const fixes = applyEdits(
      source,
      (await current.query("/workspace/entry.tsx", {
        kind: "fixes",
      }))!.edits!,
    );
    expect(fixes).toContain('const ignored=style(["flex",base,ref])');
  } finally {
    await current.close();
  }
});

test("mutated bindings and cyclic aliases delimit input grouping", async () => {
  const source = `import {style} from "stillwind/react";
const base=style("block"); const ref=style.ref(); let changed=base; changed=other;
const left=right; const right=left;
const value=style(["flex",changed,ref,"block",base,left,"p-4",ref]);`;
  const current = project(source);
  try {
    const sorted = applyEdits(
      source,
      (await current.query("/workspace/entry.tsx", {
        kind: "sort",
      }))!.edits!,
    );
    expect(sorted).toContain('style(["flex",changed,ref,base,"block",left,ref,"p-4"])');
  } finally {
    await current.close();
  }
});
