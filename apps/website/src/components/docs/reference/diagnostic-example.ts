import { mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { createRequire } from "node:module";
import { resolve } from "node:path";
import { pathToFileURL } from "node:url";

import {
  AnalysisProject,
  compileModule,
  compilerOptions,
  createFileSystemAnalysisHost,
  TypeScriptFactsService,
} from "@stillwind/host";
import {
  NativeDesignSystemLoader,
  NativeProjectSession,
  type BindingReport,
} from "@stillwind/native";
import {
  lintRules,
  resolveLintConfig,
  type AnalysisProjectConfig,
  type BaseConfig,
  type LintRule,
  type StillwindConfig,
} from "stillwind/config";
import ts from "typescript";

import { referenceCompiler } from "../../../lib/docs/reference/compiler";

export interface ExampleSource {
  language: "ts" | "tsx" | "css" | "json" | "html";
  title?: string;
  code: string;
}

export interface ExampleDiagnostic {
  code: string;
  message: string;
  notes: string[];
  labels: { source: number; start: number; end: number; message?: string }[];
}

export interface DiagnosticExample {
  sources: ExampleSource[];
  diagnostics: ExampleDiagnostic[];
  inline: boolean;
}

function section(page: string, heading: string) {
  return page.split(`## ${heading}\n`)[1]?.split(/\n## /)[0] ?? "";
}

function sources(markdown: string): ExampleSource[] {
  return [...markdown.matchAll(/```(\w+)([^\n]*)\n([\s\S]*?)\n```/g)].map(
    ([, language, meta, code]) => ({
      language: language as ExampleSource["language"],
      title: /title="([^"]+)"/.exec(meta!)?.[1],
      code: code!,
    }),
  );
}

const examples = new Map<string, Promise<DiagnosticExample>>();

export function diagnosticExample(code: string) {
  let example = examples.get(code);
  if (!example) {
    example = loadExample(code);
    examples.set(code, example);
  }
  return example;
}

async function loadExample(code: string): Promise<DiagnosticExample> {
  const inline = code in lintRules;
  const page = await readFile(
    new URL(
      `../../../content/docs/docs/${inline ? "lints" : "errors"}/${code}.mdx`,
      import.meta.url,
    ),
    "utf8",
  );
  const blocks = sources(section(page, "Reported"));
  if (!blocks.length) {
    throw new Error(`${code} requires a Reported code example`);
  }
  const diagnostics = inline
    ? await lintDiagnostics(code as LintRule, page, blocks)
    : await errorDiagnostics(code, blocks);
  if (!diagnostics.length) {
    throw new Error(`${code}: the Reported example does not produce its diagnostic`);
  }
  const displayed = inline
    ? diagnostics
    : [
        ...new Map(
          diagnostics.map((report) => [
            JSON.stringify([report.code, report.message, report.notes]),
            report,
          ]),
        ).values(),
      ];
  return { sources: blocks, diagnostics: displayed, inline };
}

const macroImport = 'import { style } from "stillwind/react";\n\n';

function script(source: ExampleSource) {
  const prefix = /import\s.*\bstyle\b/.test(source.code) ? "" : macroImport;
  return {
    code: prefix + source.code,
    prefix: prefix.length,
  };
}

function diagnostic(report: BindingReport, blocks: ExampleSource[], prefixes: number[]) {
  const labels: ExampleDiagnostic["labels"] = [];
  for (const label of [report.primary, ...report.related]) {
    const index = blocks.findIndex((block) => {
      const filename =
        block.title ?? (block.language === "css" ? "Reference.css" : "Reference.tsx");
      return decodeURI(label.location.uri).endsWith(`/${filename}`);
    });
    if (index < 0 || label.scope !== "range") {
      continue;
    }
    const start = label.location.startUtf16 - (prefixes[index] ?? 0);
    const end = label.location.endUtf16 - (prefixes[index] ?? 0);
    if (end >= 0 && start <= blocks[index]!.code.length) {
      labels.push({
        source: index,
        start: Math.max(0, start),
        end: Math.min(blocks[index]!.code.length, end),
        message: label.message,
      });
    }
  }
  return {
    code: report.code,
    message: report.message,
    notes: report.notes,
    labels,
  };
}

async function lintDiagnostics(rule: LintRule, page: string, blocks: ExampleSource[]) {
  const configured = sources(section(page, "Configuration"))[0];
  const overrides: BaseConfig = configured ? JSON.parse(configured.code) : {};
  const setting = resolveLintConfig().rules[rule];
  const severity = typeof setting === "string" ? setting : setting.severity;
  const lint: BaseConfig["lint"] = {
    rules: {
      [rule]: severity === "off" ? "warn" : setting,
      ...(typeof overrides.lint === "object" ? overrides.lint.rules : {}),
    },
  };
  const source = blocks.find((block) => block.language !== "css") ?? blocks[0]!;
  const prefixes = blocks.map((block) =>
    block.language === "css" ? '@import "stillwind";\n'.length : script(block).prefix,
  );
  if (/<LintRuleSummary[^>]+analysis="project"/.test(page)) {
    const files = Object.fromEntries(
      sources(section(page, "Project files")).map((block) => [block.title!, block.code]),
    );
    const prepared = script(source);
    const reports = await projectDiagnostics(prepared.code, { ...overrides, lint }, files);
    return reports
      .filter((report) => report.code === rule)
      .map((report) => diagnostic(report, blocks, prefixes));
  }
  const cssPrefix = '@import "stillwind";\n';
  const stylesheet = blocks.find((block) => block.language === "css");
  const compiler = await referenceCompiler(
    compilerOptions({ ...overrides, lint }),
    stylesheet ? cssPrefix + stylesheet.code : undefined,
  );
  const prepared = script(source);
  const reports =
    source.language === "css"
      ? compiler.loader.stylesheetReports()
      : new NativeProjectSession(compiler.loader, false, undefined, overrides.naming)
          .diagnose([
            {
              kind: "direct",
              moduleId: "Reference.tsx",
              document: {
                uri: "file:///Reference.tsx",
                format: "jsx",
                code: prepared.code,
              },
              syntax: {
                language: "tsx",
                moduleKind: "module",
              },
            },
          ])
          .flatMap((batch) => batch.reports);
  return reports
    .filter((report) => report.code === rule)
    .map((report) => diagnostic(report, blocks, prefixes));
}

async function projectDiagnostics(
  code: string,
  config: BaseConfig,
  files: Record<string, string> = {},
  compile = false,
  entry = "Reference.tsx",
) {
  const parent = resolve(import.meta.dirname, "../../../../.astro");
  await mkdir(parent, { recursive: true });
  const root = await mkdtemp(resolve(parent, "diagnostic-"));
  if (!root.startsWith(`${parent}/`) && !root.startsWith(`${parent}\\`)) {
    throw new Error("Diagnostic workspace escaped the Astro cache");
  }
  let project: AnalysisProject | undefined;
  let facts: TypeScriptFactsService | undefined;
  try {
    for (const [name, source] of Object.entries({
      "Reference.css": '@import "stillwind";',
      "tsconfig.json": JSON.stringify({
        compilerOptions: {
          module: "esnext",
          moduleResolution: "bundler",
          jsx: "react-jsx",
        },
        include: ["*.ts", "*.tsx"],
      }),
      ...files,
      [entry]: code,
    })) {
      await writeFile(resolve(root, name), source);
    }
    const host = createFileSystemAnalysisHost(
      { ...config, css: "Reference.css", files: ["*.tsx"] },
      root,
    );
    if (compile) {
      const compiler = await referenceCompiler(compilerOptions(config));
      facts = new TypeScriptFactsService(host.typeFactsEnvironment);
      const output = await compileModule(
        new NativeProjectSession(compiler.loader, false),
        () => facts,
        code,
        resolve(root, entry),
        host,
        {
          diagnostics: "collect",
        },
      );
      await host.close();
      return output?.reports ?? [];
    }
    project = new AnalysisProject(host);
    const result = await project.analyze(resolve(root, entry));
    if (result.error) {
      throw new Error(result.error);
    }
    return result.reports;
  } finally {
    facts?.close();
    await project?.close();
    await rm(root, { recursive: true, force: true });
  }
}

async function evaluate(source: string) {
  const require = createRequire(import.meta.url);
  for (const specifier of ["@stillwind/native", "stillwind/config"]) {
    source = source.replaceAll(
      JSON.stringify(specifier),
      JSON.stringify(pathToFileURL(require.resolve(specifier)).href),
    );
  }
  const executable = ts.transpileModule(source, {
    compilerOptions: {
      target: ts.ScriptTarget.ESNext,
      module: ts.ModuleKind.ESNext,
    },
  }).outputText;
  const url = `data:text/javascript;base64,${Buffer.from(executable).toString("base64")}`;
  const module = await import(/* @vite-ignore */ url);
  return module.default;
}

async function projectConfiguration(source: ExampleSource): Promise<AnalysisProjectConfig> {
  const config: StillwindConfig = await evaluate(source.code);
  if (config.projects.length !== 1) {
    throw new Error("A diagnostic example must configure exactly one project");
  }
  return config.projects[0]!;
}

// Native loader exceptions retain the diagnostic message but omit its reporting code.
const exceptionMessages: Record<string, string> = {
  SW01002: "prefix must start with",
  SW01003: "prefix and tailwindPrefix must be different",
  SW01004: "hashLength must be between",
  SW01006: "invalid `no-restricted-length-units` lint configuration:",
  SW01008: "the root CSS source cannot be unresolved",
  SW13001: "CSS parse error:",
  SW13002: "unterminated CSS block at byte",
  SW14001: "generated utilities require an @stillwind-slot utilities;",
  SW14002: "generated support requires an @stillwind-slot",
  SW14003: "invalid Stillwind property requirement at byte",
  SW14005: "required custom property",
  SW14007: "invalid --theme() reference at value byte",
  SW32002: "initial provide must contain exactly one file",
  SW32003: "unknown or duplicate request ID",
  SW32005: "provide called after completion",
};

async function errorDiagnostics(
  code: string,
  blocks: ExampleSource[],
): Promise<ExampleDiagnostic[]> {
  const first = blocks[0]!;
  try {
    const project =
      first.title === "stillwind.config.ts" ? await projectConfiguration(first) : undefined;
    if (["SW01002", "SW01003", "SW01004"].includes(code)) {
      new NativeDesignSystemLoader({
        prefix: project!.prefix,
        tailwindPrefix: project!.tailwindPrefix,
        hashLength: project!.hashLength,
      });
    } else if (code === "SW01006") {
      const rules = typeof project!.lint === "object" ? project!.lint.rules! : {};
      new NativeDesignSystemLoader({
        lint: Object.fromEntries(
          Object.entries(rules).map(([name, setting]) => [
            lintRules[name as LintRule].nativeName,
            setting,
          ]),
        ),
      });
    } else if (code.startsWith("SW320")) {
      await evaluate(first.code);
    } else if (code === "SW01008") {
      const loader = new NativeDesignSystemLoader();
      loader.provideMany([
        {
          id: 0,
          name: project!.css,
          content: "",
          unresolved: true,
        },
      ]);
    } else if (["SW11002", "SW12001", "SW12002", "SW10002"].includes(code)) {
      const entry = first.title ?? (code === "SW10002" ? "Reference.vue" : "Reference.tsx");
      const prepared = code === "SW10002" ? { code: first.code, prefix: 0 } : script(first);
      const files = Object.fromEntries(blocks.slice(1).map((block) => [block.title!, block.code]));
      const reports = await projectDiagnostics(prepared.code, project ?? {}, files, true, entry);
      return reports
        .filter((report) => report.code === code)
        .map((report) => diagnostic(report, [], []));
    } else {
      const css = blocks.find((block) => block.language === "css");
      const compiler = await referenceCompiler(
        compilerOptions(project ?? {}),
        css
          ? (code === "SW14001" || code === "SW14002" ? "" : '@import "stillwind";\n') + css.code
          : undefined,
      );
      let source = blocks.find(
        (block) =>
          block.title !== "stillwind.config.ts" &&
          (block.language === "tsx" || block.language === "ts"),
      );
      if (!source) {
        source = { language: "tsx", code: 'style("p-4");' };
      }
      const session = new NativeProjectSession(compiler.loader, false, undefined, project?.naming);
      const output = session.compile([
        {
          kind: "direct",
          moduleId: "Reference.tsx",
          document: {
            uri: "file:///Reference.tsx",
            format: "jsx",
            code: script(source).code,
          },
          syntax: {
            language: "tsx",
            moduleKind: "module",
          },
        },
      ]);
      const reports = output.modules
        .flatMap((module) => module.reports)
        .filter((report) => report.code === code);
      if (reports.length) {
        return reports.map((report) => diagnostic(report, [], []));
      }
      session.css();
    }
  } catch (error) {
    if (
      !(error instanceof Error) ||
      !exceptionMessages[code] ||
      !error.message.startsWith(exceptionMessages[code]!)
    ) {
      throw error;
    }
    return [
      {
        code,
        message: error.message,
        notes: [],
        labels: [],
      },
    ];
  }
  return [];
}

export function diagnosticMarkdown(example: DiagnosticExample) {
  const source = example.sources
    .map(
      (block) =>
        `${block.title ? `**${block.title}**\n\n` : ""}\`\`\`${block.language}\n${block.code}\n\`\`\``,
    )
    .join("\n\n");
  const messages = example.diagnostics
    .map((report) => {
      const locations = report.labels.map((label) => {
        const source = example.sources[label.source]!;
        const before = source.code.slice(0, label.start).split("\n");
        return `${source.title ?? "Example"}:${before.length}:${before.at(-1)!.length + 1}${label.message ? `: ${label.message}` : ""}`;
      });
      return [`${report.code}: ${report.message}`, ...locations, ...report.notes]
        .join("\n\n")
        .split("\n")
        .map((line) => `> ${line}`)
        .join("\n");
    })
    .join("\n\n");
  return `${source}\n\n${messages}`;
}
