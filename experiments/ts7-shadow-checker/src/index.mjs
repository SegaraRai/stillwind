import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

import {
  isCallExpression,
  isIdentifier,
  isPropertyAccessExpression,
  isVariableDeclaration,
} from "typescript/unstable/ast/is";
import { API } from "typescript/unstable/sync";

const experimentDir = path.dirname(path.dirname(fileURLToPath(import.meta.url)));
const fixtureDir = path.join(experimentDir, "fixture");
const configFile = path.join(experimentDir, "tsconfig.json");
const stageMs = {};

const normalize = (fileName) => path.resolve(fileName).replaceAll("\\", "/").toLowerCase();
const shadowFiles = new Map();
const api = new API({
  cwd: experimentDir,
  collectTiming: true,
  fs: {
    readFile(fileName) {
      return shadowFiles.get(normalize(fileName));
    },
  },
});

function visit(node, callback) {
  callback(node);
  node.forEachChild((child) => visit(child, callback));
}

function findProducerEdits(sourceFile) {
  const edits = [];
  visit(sourceFile, (node) => {
    if (
      !isCallExpression(node) ||
      node.typeArguments ||
      !isPropertyAccessExpression(node.expression) ||
      !isIdentifier(node.expression.expression) ||
      node.expression.expression.text !== "style" ||
      !isIdentifier(node.expression.name)
    ) {
      return;
    }

    const kind = node.expression.name.text;
    if (kind !== "scope" && kind !== "ref") return;

    const relativeFile = path.relative(fixtureDir, sourceFile.fileName).replaceAll("\\", "/");
    const originalOffset = node.getStart(sourceFile);
    const identity = `${kind}:${relativeFile}@${originalOffset}`;
    const identityLiteral = JSON.stringify(identity);
    edits.push({
      at: kind === "scope" ? node.expression.end : node.end - 1,
      text:
        kind === "scope"
          ? `<${identityLiteral}>`
          : `${node.arguments.length === 0 ? "undefined" : ""}, undefined as unknown as { readonly __stillwindDeclarationIdentity: ${identityLiteral} }`,
      kind,
      identity,
      originalOffset,
    });
  });
  return edits;
}

function applyInsertions(text, edits) {
  return [...edits]
    .sort((left, right) => right.at - left.at)
    .reduce((result, edit) => result.slice(0, edit.at) + edit.text + result.slice(edit.at), text);
}

function findProbes(sourceFile) {
  const probes = [];
  visit(sourceFile, (node) => {
    if (
      isVariableDeclaration(node) &&
      isIdentifier(node.name) &&
      node.name.text.startsWith("probe_") &&
      node.initializer
    ) {
      probes.push({ name: node.name.text, node: node.initializer });
    }
  });
  return probes;
}

function flattenRefAlternatives(checker, type) {
  const constituents = type.isUnionType() ? type.getTypes() : [type];
  const alternatives = [];

  for (const constituent of constituents) {
    if (!constituent.isTypeReference()) continue;
    const symbol = constituent.getSymbol();
    if (symbol?.name !== "Ref") continue;
    const [refType, scopeType] = checker.getTypeArguments(constituent);
    if (!refType?.isStringLiteralType()) continue;
    alternatives.push({
      ref: refType.value,
      scope: scopeType?.isStringLiteralType() ? scopeType.value : null,
    });
  }

  return alternatives.sort((left, right) => left.ref.localeCompare(right.ref));
}

let originalSnapshot;
let shadowSnapshot;
try {
  let stageStarted = performance.now();
  originalSnapshot = api.updateSnapshot({ openProjects: [configFile] });
  stageMs.originalSnapshot = performance.now() - stageStarted;
  const originalProject = originalSnapshot.getProject(configFile);
  assert(originalProject, "TS7 did not open the fixture project");
  assert.deepEqual(
    originalProject.program
      .getSemanticDiagnostics()
      .map((diagnostic) => `${diagnostic.code}: ${diagnostic.text}`),
    [],
    "original fixture must type-check",
  );

  const producerFiles = originalProject.program
    .getSourceFileNames()
    .filter((fileName) => normalize(fileName).startsWith(`${normalize(fixtureDir)}/`))
    .filter((fileName) => fileName.endsWith(".ts"));
  const producerEdits = [];

  for (const fileName of producerFiles) {
    const sourceFile = originalProject.program.getSourceFile(fileName);
    assert(sourceFile, `missing source file: ${fileName}`);
    const edits = findProducerEdits(sourceFile);
    if (edits.length === 0) continue;
    shadowFiles.set(normalize(fileName), applyInsertions(sourceFile.text, edits));
    producerEdits.push(...edits.map((edit) => ({ fileName, ...edit })));
  }

  assert.equal(producerEdits.length, 4, "expected two scopes and two refs");
  const changed = producerEdits.map((edit) => edit.fileName);
  stageStarted = performance.now();
  shadowSnapshot = api.updateSnapshot({ fileChanges: { changed: [...new Set(changed)] } });
  const shadowProject = shadowSnapshot.getProject(configFile);
  assert(shadowProject, "TS7 did not retain the fixture project");

  const diagnostics = shadowProject.program.getSemanticDiagnostics();
  assert.deepEqual(
    diagnostics.map((diagnostic) => `${diagnostic.code}: ${diagnostic.text}`),
    [],
    "shadow source must type-check",
  );
  stageMs.shadowSnapshotAndCheck = performance.now() - stageStarted;

  const consumerFile = shadowProject.program
    .getSourceFileNames()
    .find((fileName) => normalize(fileName) === normalize(path.join(fixtureDir, "consumer.ts")));
  assert(consumerFile, "consumer.ts is not part of the project");
  const consumer = shadowProject.program.getSourceFile(consumerFile);
  assert(consumer, "missing consumer.ts");

  stageStarted = performance.now();
  const results = findProbes(consumer).map(({ name, node }) => {
    const type = shadowProject.checker.getTypeAtLocation(node);
    assert(type, `missing type for ${name}`);
    return {
      name,
      type: shadowProject.checker.typeToString(type, node),
      alternatives: flattenRefAlternatives(shadowProject.checker, type),
    };
  });

  const direct = results.find((result) => result.name === "probe_direct");
  assert.equal(direct?.alternatives.length, 2, "conditional must preserve both ref candidates");
  for (const result of results) {
    assert.deepEqual(
      result.alternatives,
      direct.alternatives,
      `${result.name} must preserve both correlated candidates`,
    );
  }
  stageMs.probeQueries = performance.now() - stageStarted;
  assert(
    direct.alternatives.every((alternative) => alternative.scope !== null),
    "each ref candidate must retain its correlated scope identity",
  );

  const output = {
    typescript: JSON.parse(
      fs.readFileSync(
        path.join(experimentDir, "../../node_modules/typescript/package.json"),
        "utf8",
      ),
    ).version,
    transport: "typescript/unstable/sync -> spawned tsgo IPC",
    shadowFiles: shadowFiles.size,
    stageMs,
    injectedProducers: producerEdits.map(({ fileName, kind, identity, originalOffset }) => ({
      file: path.relative(experimentDir, fileName).replaceAll("\\", "/"),
      kind,
      identity,
      originalOffset,
    })),
    probes: results,
    timing: api.getTimingInfo(),
  };

  process.stdout.write(`${JSON.stringify(output, null, 2)}\n`);
} finally {
  shadowSnapshot?.dispose();
  originalSnapshot?.dispose();
  api.close();
}
