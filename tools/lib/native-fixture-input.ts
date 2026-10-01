import path from "node:path";
import { pathToFileURL } from "node:url";

import {
  compileSource,
  type NativeCompileCssInput,
  type NativeGeneratedClass,
  type NativeProjectSource,
} from "@stillwind/native";

export function fixtureProjectSource(source: string, moduleId: string): NativeProjectSource {
  const uri = pathToFileURL(path.resolve(moduleId)).href;
  return {
    moduleId: moduleId,
    uri,
    sourceMapSource: uri,
    language: moduleId.endsWith(".tsx") ? "tsx" : "ts",
    moduleKind: "unambiguous",
    source,
  };
}

export function fixtureCss(content: string): NativeCompileCssInput {
  const uri = pathToFileURL(path.resolve("stillwind.css")).href;
  return { content, name: "stillwind.css", uri, sourceMapSource: uri };
}

export function compileFixtureSource(source: string, moduleId: string, css: string) {
  return compileSource({ ...fixtureProjectSource(source, moduleId), css: fixtureCss(css) });
}

export function comparableFixtureClasses(classes: NativeGeneratedClass[]) {
  return classes.map(({ callSpan, guardPath, ...generated }) => ({
    ...generated,
    callSpan: { start: callSpan.start, end: callSpan.end },
    guardPath: guardPath.map(({ test, ...guard }) => ({
      ...guard,
      test: { start: test.start, end: test.end },
    })),
  }));
}
