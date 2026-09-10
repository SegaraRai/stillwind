function build(language, name, script, template, extra = "", probes = []) {
  const source =
    language === "astro"
      ? `---\n${script}\n---\n${template}\n${extra}`
      : language === "svelte"
        ? `${extra}<script lang="ts">\n${script}\n</script>\n${template}`
        : `${extra}<script setup lang="ts">\n${script}\n</script>\n<template>${template}</template>`;
  return { language, name, source, probes };
}
function importProbe(raw, module = "stillwind/react") {
  return { kind: "import", raw, module };
}
function callProbe(raw, marker, region = "script") {
  return { kind: "call", raw, marker, region };
}
const macroImport = 'import { style } from "stillwind/react";';
export const fixtures = [];
for (const language of ["astro", "svelte", "vue"]) {
  const template = (raw) =>
    language === "vue" ? `<div v-bind="${raw}"/>` : `<div {...${raw}}></div>`;
  for (const [name, prefix, scriptCall, templateCall] of [
    ["basic", "", "style('script-basic')", "style('template-basic')"],
    [
      "unicode",
      '// 日本語 🧭\nconst label="é🧭"; ',
      "style('script-é🧭')",
      "style('template-中🧭')",
    ],
    [
      "multiline",
      "",
      "style(\n /* 日本語 🧭 */\n 'script-multi'\n)",
      "style(\n 'template-multi'\n)",
    ],
    ["generic", "", "style<{label:string}>('script-generic')", "style('template-generic')"],
  ]) {
    const markers = [...scriptCall.matchAll(/'(.*?)'/g)].map((x) => x[1]);
    const tmarker = [...templateCall.matchAll(/'(.*?)'/g)][0][1];
    fixtures.push(
      build(
        language,
        name,
        `${prefix}${macroImport}\nconst props = ${scriptCall};`,
        template(templateCall),
        "",
        [
          importProbe(macroImport),
          callProbe(scriptCall, markers[0]),
          callProbe(templateCall, tmarker, "template"),
        ],
      ),
    );
  }
  const aliasImport = 'import {\n /* alias 🧭 */ style as sx,\n} from "stillwind/react";';
  fixtures.push(
    build(
      language,
      "alias-crlf",
      `${aliasImport}\nconst props = sx('script-alias');`,
      template("sx('template-alias')"),
      "",
      [
        importProbe(aliasImport),
        callProbe("sx('script-alias')", "script-alias"),
        callProbe("sx('template-alias')", "template-alias", "template"),
      ],
    ),
  );
  const cssImport = 'import { css as sheet } from "stillwind/module-css";';
  fixtures.push(
    build(
      language,
      "module-css",
      `${macroImport}\n${cssImport}\nconst props = style('sheet-style');`,
      template("props"),
      "",
      [
        importProbe(macroImport),
        importProbe(cssImport, "stillwind/module-css"),
        callProbe("style('sheet-style')", "sheet-style"),
      ],
    ),
  );
  fixtures.push(
    build(
      language,
      "duplicates",
      `${macroImport}\nconst a = style('same');\nconst b = style('same');`,
      template("style('same')"),
      "",
      [
        importProbe(macroImport),
        ...[0, 1, 2].map((occurrence) => ({
          ...callProbe("style('same')", "same", occurrence === 2 ? "template" : "script"),
          occurrence,
        })),
      ],
    ),
  );
  fixtures.push(
    build(
      language,
      "scoped-expression",
      `${macroImport}\nconst items = [{tone:'red'}];`,
      language === "vue"
        ? `<div v-for="item in items" v-bind="style('template-scope', item.tone)"/>`
        : language === "svelte"
          ? `{#each items as item}<div {...style('template-scope', item.tone)}></div>{/each}`
          : `{items.map(item => <div {...style('template-scope', item.tone)}></div>)}`,
      "",
      [
        importProbe(macroImport),
        callProbe("style('template-scope', item.tone)", "template-scope", "template"),
      ],
    ),
  );
}
fixtures.push(
  build(
    "svelte",
    "two-scripts",
    `${macroImport}\nconst props=style('instance');`,
    "<div {...props}></div>",
    `<script module lang="ts">\nimport { style as sx } from "stillwind/react";\nexport const moduleProps=sx('module');\n</script>`,
    [
      importProbe(macroImport),
      importProbe('import { style as sx } from "stillwind/react";'),
      callProbe("sx('module')", "module", "module-script"),
      callProbe("style('instance')", "instance"),
    ],
  ),
);
fixtures.push(
  build(
    "astro",
    "client-script",
    `${macroImport}\nconst props=style('server');`,
    "<div {...props}></div>",
    `<script>\nimport { style as sx } from "stillwind/react";\nconst clientProps=sx('client');\n</script>`,
    [
      importProbe(macroImport),
      importProbe('import { style as sx } from "stillwind/react";'),
      callProbe("style('server')", "server"),
      callProbe("sx('client')", "client", "client-script"),
    ],
  ),
);
fixtures.push(
  build(
    "vue",
    "two-scripts",
    `${macroImport}\nconst props=style('setup');`,
    '<div v-bind="props"/>',
    `<script lang="ts">\nimport { style as sx } from "stillwind/react";\nexport const shared=sx('normal');\nexport default {name:'Two'};\n</script>`,
    [
      importProbe(macroImport),
      importProbe('import { style as sx } from "stillwind/react";'),
      callProbe("style('setup')", "setup"),
      callProbe("sx('normal')", "normal", "normal-script"),
    ],
  ),
);
for (const fixture of fixtures) {
  if (fixture.name === "alias-crlf") {
    fixture.source = fixture.source.replaceAll("\n", "\r\n");
    fixture.probes = fixture.probes.map((p) => ({ ...p, raw: p.raw.replaceAll("\n", "\r\n") }));
  }
  for (const probe of fixture.probes) {
    let index = -1;
    for (let i = 0; i <= (probe.occurrence ?? 0); i++)
      index = fixture.source.indexOf(probe.raw, index + 1);
    if (index < 0) throw new Error(`missing probe ${fixture.name}: ${probe.raw}`);
    probe.start = index;
    probe.end = index + probe.raw.length;
  }
}
