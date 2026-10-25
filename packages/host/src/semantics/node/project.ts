import { createHash } from "node:crypto";
import { existsSync } from "node:fs";
import path from "node:path";

import type {
  NativeAssetRecord as AssetRecord,
  NativeIdentity as IdentityId,
} from "@stillwind/native";

import { TypeFactsError, type TypeFactSource } from "../backend.ts";
import { CheckerSource, checkerId } from "../checker-source.ts";

/**
 * Normalizes filesystem names used as keys in the virtual checker overlay.
 *
 * @param fileName - Absolute or relative filesystem path.
 * @returns An absolute path with forward slashes, lowercased on Windows to match case-insensitive
 * filesystem lookups.
 */
export const normalize = (fileName: string) => {
  const name = path.resolve(fileName).replaceAll("\\", "/");
  return process.platform === "win32" ? name.toLowerCase() : name;
};

/**
 * Virtual project files, checker range mappings, and declaration identities prepared for one
 * collection request.
 */
export interface PreparedProject {
  /**
   * Normalized absolute filenames mapped to virtual text, including checker sources, helpers,
   * globals, and project configuration.
   */
  virtualFiles: Map<string, string>;
  /**
   * Checker buffers in source and buffer order, retaining the correspondence to original probes.
   */
  mappings: CheckerSource[];
  /**
   * Serialized identity keys collected from all checker buffers, mapped to their native
   * declarations.
   */
  identities: Map<string, IdentityId>;
}

/**
 * TypeScript settings inherited by the generated project used for semantic queries.
 */
export type TypeScriptProject =
  | {
      /** Selects settings inherited from a consumer tsconfig. */
      readonly kind: "configured";
      /** Absolute path to the consumer's tsconfig. */
      readonly configPath: string;
    }
  | {
      /** Selects Stillwind's defaults without a consumer tsconfig. */
      readonly kind: "isolated";
    };

/**
 * Fixed filesystem and TypeScript settings shared by collections in one checker service.
 */
export interface TypeFactsEnvironment {
  /** Absolute directory used for checker resolution and generated configuration paths. */
  readonly root: string;
  /** Settings inherited by the checker. Isolated projects use Stillwind's compiler defaults. */
  readonly project: TypeScriptProject;
}

/**
 * Selects the TypeScript environment before starting a checker.
 *
 * @param root - Filesystem project directory, resolved to an absolute path.
 * @param tsconfig - Absolute or root-relative config path. Omission selects an existing root
 * `tsconfig.json`. False selects isolated defaults even when that file exists.
 * @returns Fixed settings without evaluating configuration or starting the checker.
 * @throws If an explicitly selected path is empty or its configuration file does not exist.
 */
export function resolveTypeFactsEnvironment(
  root: string,
  tsconfig?: string | false,
): TypeFactsEnvironment {
  root = path.resolve(root);
  if (typeof tsconfig === "string" && !tsconfig.trim()) {
    throw new Error("TypeScript configuration path must not be empty");
  }
  const configPath = path.resolve(root, typeof tsconfig === "string" ? tsconfig : "tsconfig.json");
  if (typeof tsconfig === "string" && !existsSync(configPath)) {
    throw new Error(`TypeScript configuration does not exist: ${configPath}`);
  }
  return {
    root,
    project:
      tsconfig !== false && existsSync(configPath)
        ? {
            kind: "configured",
            configPath,
          }
        : {
            kind: "isolated",
          },
  };
}

/**
 * File contents captured for one collection. Ambient declarations override overlays at matching
 * paths. Prepared checker sources and the generated configuration override both maps.
 */
export interface TypeFactsInputs {
  /** Absolute paths to replacement text. Files participate only when the checker references them. */
  overlayFiles?: ReadonlyMap<string, string>;
  /** Absolute declaration paths to text. Every supplied declaration becomes a project root file. */
  ambientDeclarations?: ReadonlyMap<string, string>;
}

/**
 * Builds checker sources and an in-memory project configuration for semantic queries.
 *
 * @param sources - Prepared sources with validated mappings, original probes, and dependency
 * resolutions.
 * @param assetsByModuleId - Native declaration records keyed by compiler module identity.
 * @param shadowTsconfigPath - Absolute filename of the generated in-memory TypeScript project.
 * @param project - Consumer configuration to inherit, or isolated compiler defaults.
 * @param inputs - Captured file replacements and declarations required by this collection.
 * @param configuredDeclarations - Declaration files selected by the consumer's tsconfig.
 * @returns Virtual text and range mappings without writing any files to disk.
 * @throws If checker edits or projection maps are invalid, or an original probe has no
 * checker-buffer correspondence.
 */
export function prepareProject(
  sources: readonly TypeFactSource[],
  assetsByModuleId: ReadonlyMap<string, readonly AssetRecord[]>,
  shadowTsconfigPath: string,
  project: TypeScriptProject,
  inputs: TypeFactsInputs,
  configuredDeclarations: readonly string[],
): PreparedProject {
  const virtualFiles = new Map(
    [...(inputs.overlayFiles ?? []), ...(inputs.ambientDeclarations ?? [])].map(([name, text]) => [
      normalize(name),
      text,
    ]),
  );
  const mappings: CheckerSource[] = [];
  const allIdentities = new Map<string, IdentityId>();
  const globals = new Set<string>();
  const ids = new Map(sources.map((source) => [source.resolverId, checkerId(source)]));
  for (const source of sources) {
    const declarations = source.kind === "projected" && source.projection.checkerGlobals;
    if (declarations) {
      const fingerprint = createHash("sha256").update(declarations).digest("hex").slice(0, 16);
      const id = normalize(
        path.join(path.dirname(shadowTsconfigPath), `.stillwind-globals-${fingerprint}.d.ts`),
      );
      virtualFiles.set(id, declarations);
      globals.add(id);
    }
    const syntax = source.kind === "direct" ? source.syntax : source.projection.syntax;
    const buffers = source.checkerBuffers ?? [
      {
        source: source.kind === "direct" ? source.document.code : source.projection.code,
        language:
          syntax.language === "tsx" || syntax.language === "jsx"
            ? ("tsx" as const)
            : ("ts" as const),
      },
    ];
    for (const [index, buffer] of buffers.entries()) {
      const shadow = new CheckerSource(
        source,
        buffer,
        index,
        assetsByModuleId.get(source.moduleId) ?? [],
        ids,
      );
      virtualFiles.set(normalize(shadow.id), shadow.text);
      if (shadow.helperModule) {
        virtualFiles.set(normalize(shadow.helperModule.id), shadow.helperModule.text);
      }
      mappings.push(shadow);
      for (const [key, identity] of shadow.identities) {
        allIdentities.set(key, identity);
      }
    }
    for (const probe of source.probes) {
      if (
        !mappings.some(
          (mapping) =>
            mapping.source === source &&
            [...mapping.probes.values()].some(
              (candidate) => candidate.start === probe.start && candidate.end === probe.end,
            ),
        )
      ) {
        throw new TypeFactsError(
          source.moduleId,
          probe.start,
          probe.end,
          "checker",
          "Cannot map this expression to a checker source",
        );
      }
    }
  }
  virtualFiles.set(
    normalize(shadowTsconfigPath),
    JSON.stringify({
      ...(project.kind === "configured"
        ? {
            extends: project.configPath,
          }
        : {}),
      compilerOptions: {
        allowJs: true,
        checkJs: true,
        ...(project.kind === "isolated"
          ? {
              jsx: "preserve",
              module: "esnext",
              moduleResolution: "bundler",
              strict: true,
              target: "esnext",
              types: [],
            }
          : {}),
        noEmit: true,
        skipLibCheck: true,
        allowImportingTsExtensions: true,
      },
      files: [
        ...configuredDeclarations,
        ...[...(inputs.ambientDeclarations?.keys() ?? [])].map(normalize),
        ...globals,
        ...mappings.map((mapping) => mapping.id),
        ...new Set(sources.flatMap((source) => source.checkerTypes ?? [])),
      ],
      include: [],
      exclude: [],
    }),
  );
  return {
    virtualFiles,
    mappings,
    identities: allIdentities,
  };
}
