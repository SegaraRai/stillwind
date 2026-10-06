import path from "node:path";
import { pathToFileURL } from "node:url";

import { GenMapping, addSegment, setSourceContent, toEncodedMap } from "@jridgewell/gen-mapping";
import { fingerprint, NativeDesignSystemLoader, NativeProjectSession } from "@stillwind/native";
import { prepareSource } from "@stillwind/projection";
import { expect, test } from "vite-plus/test";

import { mapAnalysisReports } from "./diagnostic-map.ts";

test("loader maps preserve native revisions and UTF-16 ranges; unmapped edits are discarded", async () => {
  const file = path.join(import.meta.dirname, "original.tsx");
  const uri = pathToFileURL(file).href;
  const original =
    'import {style} from "stillwind/react"; const 日本語="🧭";export const props=style("missing");';
  const generated = "\n" + original;
  const map = new GenMapping();
  setSourceContent(map, uri, original);
  for (let column = 0; column <= original.length; column++) {
    addSegment(map, 1, column, uri, 0, column);
  }
  const source = await prepareSource({
    resolverId: file,
    moduleId: "original.tsx",
    checkerFileName: file,
    projectionFileName: file,
    document: {
      uri,
      code: generated,
      format: "jsx",
      upstreamMap: JSON.stringify(toEncodedMap(map)),
    },
    syntax: {
      language: "tsx",
      moduleKind: "module",
    },
  });
  const loader = new NativeDesignSystemLoader();
  loader.provide(0, "root.css", "@stillwind-slot utilities;");
  const report = new NativeProjectSession(loader, false).compile([source]).modules[0].reports[0];
  expect(report.primary.location.revision).toBe(fingerprint(generated));
  report.actions = [
    {
      title: "Replace utility",
      edits: [
        {
          location: report.primary.location,
          replacement: "red",
        },
      ],
    },
  ];
  const mapped = (await mapAnalysisReports([report], [source], async () => original))[0];
  expect(mapped.primary).toMatchObject({
    scope: "range",
    location: {
      uri,
      revision: fingerprint(original),
      startLine: 0,
      startColumn: original.indexOf("missing"),
    },
  });
  expect(mapped.actions[0].edits[0].location.revision).toBe(fingerprint(original));
  source.document = {
    ...source.document,
    upstreamMap: JSON.stringify({
      version: 3,
      sources: [uri],
      sourcesContent: [original],
      names: [],
      mappings: "",
    }),
  };
  const unmapped = (await mapAnalysisReports([report], [source], async () => original))[0];
  expect(unmapped.primary).toMatchObject({
    scope: "document",
    location: {
      uri,
      startLine: 0,
      startColumn: 0,
    },
  });
  expect(unmapped.actions).toEqual([]);
});
