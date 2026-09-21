import * as astro from "@astrojs/compiler";
import * as astroRs from "@astrojs/compiler-rs";
import { parse as parseVue, compileScript } from "@vue/compiler-sfc";
import * as vue from "@vue/language-core";
import { compile as compileSvelte } from "svelte/compiler";
import { svelte2tsx } from "svelte2tsx";
import ts from "typescript";

export const adapters = [
  {
    name: "astro-tsx",
    language: "astro",
    async convert(source, filename) {
      return { ...(await astro.convertToTSX(source, { filename })), extension: ".tsx" };
    },
  },
  {
    name: "astro-tsx-no-scripts",
    language: "astro",
    async convert(source, filename) {
      return {
        ...(await astro.convertToTSX(source, { filename, includeScripts: false })),
        extension: ".tsx",
      };
    },
  },
  {
    name: "astro-rs-tsx",
    language: "astro",
    convert(source, filename) {
      return { ...astroRs.convertToTSX(source, { filename }), extension: ".tsx" };
    },
  },
  {
    name: "astro-runtime",
    language: "astro",
    async convert(source, filename) {
      return {
        ...(await astro.transform(source, { filename, sourcemap: "external" })),
        extension: ".tsx",
      };
    },
  },
  {
    name: "astro-rs-runtime",
    language: "astro",
    convert(source, filename) {
      return {
        ...astroRs.transform(source, { filename, sourcemap: "external" }),
        extension: ".js",
      };
    },
  },
  {
    name: "svelte2tsx",
    language: "svelte",
    convert(source, filename) {
      return {
        ...svelte2tsx(source, { filename, isTsFile: true, emitOnTemplateError: false }),
        extension: ".tsx",
      };
    },
  },
  ...["client", "server"].map((generate) => ({
    name: `svelte-runtime-${generate}`,
    language: "svelte",
    convert(source, filename) {
      return { ...compileSvelte(source, { filename, generate }).js, extension: ".js" };
    },
  })),
  {
    name: "vue-language-core",
    language: "vue",
    convert(source, filename) {
      const plugin = vue.createVueLanguagePlugin(
        ts,
        {},
        vue.getDefaultCompilerOptions(),
        (id) => id,
      );
      const root = plugin.createVirtualCode(filename, "vue", ts.ScriptSnapshot.fromString(source));
      const service = plugin.typescript.getServiceScript(root);
      const code = service.code;
      const result = {
        code: code.snapshot.getText(0, code.snapshot.getLength()),
        extension: service.extension,
        mappings: code.mappings,
      };
      plugin.disposeVirtualCode(filename);
      return result;
    },
  },
  ...[false, true].map((inlineTemplate) => ({
    name: `vue-compile-script${inlineTemplate ? "-inline" : ""}`,
    language: "vue",
    convert(source, filename) {
      const parsed = parseVue(source, { filename });
      if (parsed.errors.length) {
        throw new Error(parsed.errors.join("\n"));
      }
      const result = compileScript(parsed.descriptor, {
        id: "research",
        inlineTemplate,
        sourceMap: true,
      });
      return { code: result.content, map: result.map, extension: ".tsx" };
    },
  })),
];
