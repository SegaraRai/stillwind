import type { PreparedSource, ProjectInput } from "@stillwind/projection";

import type { ModuleDescriptor } from "./module-source.ts";

/**
 * Source metadata, dependency resolution, reads, watches, and warnings supplied to
 * module compilation. Hosts must preserve a stable correspondence between resolver and
 * compiler module identities.
 */
export interface CompilerHost {
  /** Requests checker import resolutions. A non-null host resolve result still takes precedence. */
  typeFactsResolution?: boolean;
  /**
   * Loads source before or after preparation, including its original document and projection.
   *
   * @param id - Full resolver identity. The returned resolverId must equal this value.
   * @returns Original input for preparation or a prepared source. Omission uses describe and read.
   * @throws If loading or preparation fails.
   */
  load?(id: string): Promise<ProjectInput | PreparedSource>;
  /**
   * Determines whether a resolved dependency supplies source that Stillwind can analyze.
   *
   * @param id - Full resolved identity, including query suffixes.
   * @returns Whether source traversal can continue. Omission checks for supported JS/TS and SFC extensions.
   */
  supports?(id: string): boolean;
  /**
   * Describes one resolved source before it is scanned or compiled.
   *
   * @param id - Full resolver identity of the source, including any query suffix.
   * @returns Compiler and checker identities, parser metadata, and original document metadata.
   */
  describe(id: string): ModuleDescriptor;
  /**
   * Resolves a semantic dependency relative to its importing source.
   *
   * @param specifier - Authored module request text.
   * @param importer - Full resolver identity of the importing source.
   * @param options - Optional resolver policy. Compilation passes `skipSelf: true`.
   * @returns A promise for a resolved identity and optional external status, or null when
   * unresolved. Unresolved and external modules are skipped by semantic dependency traversal.
   */
  resolve(
    specifier: string,
    importer: string,
    options?: {
      /** Requests resolution without calling the initiating plugin again. */
      skipSelf: boolean;
    },
  ): Promise<{
    /** Full host identity used to load the resolved source. */
    id: string;
    /** Truthy values mark a dependency that the host does not load as source. */
    external?: boolean | string;
  } | null>;
  /**
   * Reads one resolved dependency before source preparation.
   *
   * @param id - Full resolver identity, including any query selecting transformed source.
   * @returns A promise for source text. Read failures reject the compilation.
   */
  read(id: string): Promise<string>;
  /**
   * Records the identity correspondence before a source is prepared.
   *
   * This callback can run before preparation fails. Callers must not treat it as proof of a
   * successful compilation.
   *
   * @param moduleId - Stable identity used by the compiler and checker results.
   * @param resolverId - Full host identity used to resolve and read this source.
   */
  onModule?(moduleId: string, resolverId: string): void;
  /**
   * Registers a resolved semantic dependency for change tracking.
   *
   * @param id - Full dependency identity selected by the host resolver.
   */
  watch(id: string): void;
  /**
   * Reports a lint warning or a rejected generated name through the host.
   *
   * @param message - Formatted warning text. No structured diagnostic location is implied.
   */
  warn(message: string): void;
}

/**
 * Loads original or prepared source through the host, reading its descriptor once.
 *
 * @param host - Host whose current buffers supply this source.
 * @param id - Full resolver identity. The host must preserve this identity in its answer.
 * @returns Prepared source from load, or an original input assembled from describe and read.
 * @throws If loading, describing, or reading the source fails.
 */
export async function loadSource(
  host: CompilerHost,
  id: string,
): Promise<ProjectInput | PreparedSource> {
  if (host.load) {
    return host.load(id);
  }

  const descriptor = host.describe(id);
  return {
    resolverId: id,
    ...descriptor,
    document: {
      ...descriptor.document,
      code: await host.read(id),
    },
  };
}
