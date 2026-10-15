import path from "node:path";
import { pathToFileURL } from "node:url";

import {
  compileSource,
  type NativeCompileCssInput,
  type NativeCompilerOptions,
  type NativeGeneratedClass,
  type NativePreparedSource,
} from "@stillwind/native";

export function fixturePreparedSource(source: string, moduleId: string): NativePreparedSource {
  const uri = pathToFileURL(path.resolve(moduleId)).href;
  return {
    kind: "direct",
    moduleId,
    document: {
      uri,
      code: source,
      format: moduleId.endsWith(".tsx") ? "jsx" : "script",
      sourceMapSource: uri,
    },
    syntax: {
      language: moduleId.endsWith(".tsx") ? "tsx" : "ts",
      moduleKind: "unambiguous",
    },
  };
}

export function fixtureCss(content: string): NativeCompileCssInput {
  const uri = pathToFileURL(path.resolve("stillwind.css")).href;
  return {
    content,
    name: "stillwind.css",
    uri,
    sourceMapSource: uri,
  };
}

export function compileFixtureSource(
  source: string,
  moduleId: string,
  css: string,
  options?: NativeCompilerOptions,
) {
  const uri = pathToFileURL(path.resolve(moduleId)).href;
  return compileSource(
    {
      source,
      moduleId,
      uri,
      sourceMapSource: uri,
      language: moduleId.endsWith(".tsx") ? "tsx" : "ts",
      moduleKind: "unambiguous",
      css: fixtureCss(css),
    },
    options,
  );
}

export function comparableFixtureClasses(classes: NativeGeneratedClass[]) {
  return classes.map(({ callSpan, guardPath, ...generated }) => ({
    ...generated,
    callSpan: {
      start: callSpan.start,
      end: callSpan.end,
    },
    guardPath: guardPath.map(({ test, ...guard }) => ({
      ...guard,
      test: {
        start: test.start,
        end: test.end,
      },
    })),
  }));
}
