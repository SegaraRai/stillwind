import { createRequire } from "node:module";
import { dirname, relative } from "node:path";

import {
  NativeDesignSystemLoader,
  type NativeCssFileInput,
  type NativeCssLoadStep,
  type NativeCssRequest,
} from "@stillwind/native";

import { referenceCompiler } from "../reference/compiler";
import { importKey, type RecordedStylesheet, type StylesheetSource } from "./stylesheet";

const require = createRequire(import.meta.url);

let recording: Promise<RecordedStylesheet> | undefined;

/**
 * Records the sources of the default stylesheet as the reference compiler loads them.
 * Source names are relative to the `stillwind` package, and the recording contains no file URIs.
 */
export function recordDefaultStylesheet(): Promise<RecordedStylesheet> {
  recording ??= record();
  return recording;
}

async function record(): Promise<RecordedStylesheet> {
  const loader = new NativeDesignSystemLoader({});
  const requests = new Map<number, NativeCssRequest>();
  const sources = new Map<string, StylesheetSource>();
  let root: StylesheetSource | undefined;
  const observe = (step: NativeCssLoadStep) => {
    if (step.kind === "needFile") {
      for (const request of step.requests) {
        requests.set(request.id, request);
      }
    }
    return step;
  };
  const accept = ({ id, ...source }: NativeCssFileInput) => {
    if (id === 0) {
      root = source;
    } else {
      sources.set(importKey(requests.get(id)!), source);
    }
  };
  const recorder = new Proxy(loader, {
    get(target, key) {
      if (key === "provide") {
        return (...args: Parameters<NativeDesignSystemLoader["provide"]>) => {
          const [id, name, content, uri, sourceMapSource, lint] = args;
          accept({
            id,
            name,
            content,
            uri: uri ?? undefined,
            sourceMapSource: sourceMapSource ?? undefined,
            lint: lint ?? undefined,
          });
          return observe(target.provide(...args));
        };
      }
      if (key === "provideMany") {
        return (files: NativeCssFileInput[]) => {
          files.forEach(accept);
          return observe(target.provideMany(files));
        };
      }
      const value = Reflect.get(target, key);
      return typeof value === "function" ? value.bind(target) : value;
    },
  });
  await referenceCompiler({}, undefined, recorder);

  const base = dirname(require.resolve("stillwind/package.json"));
  const publicName = (name: string) =>
    name === root!.name
      ? "utilities.css"
      : `stillwind/${relative(base, name).replaceAll("\\", "/")}`;
  const rename = ({ name, content, lint, origin, unresolved }: StylesheetSource) => ({
    name: unresolved ? name : publicName(name),
    content,
    lint,
    origin,
    unresolved,
  });
  return {
    root: rename(root!),
    imports: Object.fromEntries(
      [...sources].map(([key, source]) => {
        const [importer, specifier] = key.split("\n") as [string, string];
        return [
          importKey({
            importer: publicName(importer),
            specifier,
          }),
          rename(source),
        ];
      }),
    ),
  };
}
