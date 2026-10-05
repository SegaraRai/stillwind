import type { EncodedSourceMap } from "@jridgewell/gen-mapping";
import type { SourceMap } from "@jridgewell/remapping";
import type { SourceMapInput } from "@jridgewell/trace-mapping";
import type { UpstreamSourceMap } from "@stillwind/host";
import type { NativeStylesheetPlan } from "@stillwind/native";
import type { LoaderContext } from "webpack";

/** Stillwind metadata passed between loaders while preserving other plugins' additional fields. */
export interface LoaderAdditionalData {
  /** AST supplied by a preceding webpack loader, when available. */
  webpackAST?: object;
  /** Compiler output and identity of a source already transformed by Stillwind. */
  stillwind?: Partial<Pick<NativeStylesheetPlan, "coverage" | "designSystem" | "producers">> & {
    /** Resource identity and digest used to avoid compiling an identical transform twice. */
    sourceTransform?: {
      /** Resource path and query that selected the source. */
      resource: string;
      /** SHA-256 hex digest of the transformed source. */
      fingerprint: string;
    };
  };
  /** Data owned by other loaders, forwarded without interpretation. */
  [key: string]: unknown;
}

/** Transformed SFC text and its map shared through the pitch data of one loader. */
export interface SourceTransform {
  /** Source after Stillwind compilation and finalization. */
  code: string;
  /** Map from transformed code back to the original component. */
  map: EncodedSourceMap | SourceMap;
  /** Resource identity that must match the source-map bridge's resource path. */
  resource: string;
}

/** Pitch data retained for source-map composition after framework loaders have run. */
export interface SourceTransformData {
  /** Stillwind transform for this loader invocation, when an SFC was compiled. */
  stillwindSourceTransform?: SourceTransform;
}

/** Loader context with typed pitch data for both webpack and Rspack's loader runner. */
export type SourceTransformContext<Options> = Omit<LoaderContext<Options>, "data" | "loaders"> & {
  /** Data shared by this loader's pitch and normal phases. */
  data: SourceTransformData;
  /** Loader chain whose entries expose pitch data in either runner's representation. */
  loaders: (Omit<LoaderContext<Options>["loaders"][number], "data"> & {
    /** Pitch data exposed directly by webpack. */
    data?: SourceTransformData;
    /** Pitch data exposed through Rspack 2's loader item. */
    loaderItem?: {
      data?: SourceTransformData;
    };
  })[];
};

/**
 * Completes a loader invocation with maps and optional metadata accepted by the loader runners.
 *
 * @param error - Failure, or null or undefined for successful completion.
 * @param source - Generated or unchanged source text.
 * @param map - Upstream map or map chain supplied by loaders, when available.
 * @param data - Additional data forwarded to the next loader, when available.
 */
export type LoaderCompletion = (
  error?: unknown,
  source?: string | Buffer,
  map?: UpstreamSourceMap | EncodedSourceMap | SourceMap,
  data?: LoaderAdditionalData,
) => void;

/**
 * Obtains an asynchronous completion callback for the shared loader protocol.
 *
 * @param context - webpack-compatible loader context for the current invocation.
 * @returns The runner's callback. Stillwind metadata does not require a `webpackAST` field, and
 * composed maps can contain nullable source content allowed by the source-map library.
 */
export function loaderCompletion(context: Pick<LoaderContext<unknown>, "async">): LoaderCompletion {
  return context.async() as LoaderCompletion;
}

/**
 * Composes a framework's output map through the Stillwind transform stored in pitch data.
 *
 * @param this - Loader context whose chain exposes the preceding Stillwind transform.
 * @param source - Framework output forwarded unchanged.
 * @param inputMap - Framework map, when available.
 * @param additionalData - Additional loader data forwarded unchanged.
 * @returns Undefined after scheduling or completing the asynchronous callback.
 */
export default function webpackSourceMapLoader(
  this: SourceTransformContext<unknown>,
  source: string,
  inputMap: SourceMapInput | null | undefined,
  additionalData?: LoaderAdditionalData,
) {
  const callback = loaderCompletion(this);

  // Vue development script maps include the template. Replacing their embedded source
  // changes script hashes on template edits and makes Vue remount the component.
  if (
    this.mode !== "production" &&
    this._module?.matchResource &&
    /[?&]vue&type=script(?:&|$)/.test(this.resourceQuery ?? "")
  ) {
    return callback(null, source, inputMap, additionalData);
  }

  // Rspack 2 stores chain data on loaderItem; webpack exposes it directly.
  // Keep this compatibility boundary local and cover both official runners.
  const transform = this.loaders
    .map((loader) => (loader.data ?? loader.loaderItem?.data)?.stillwindSourceTransform)
    .find((entry) => entry?.resource === this.resourcePath);
  if (!inputMap || !transform) {
    return callback(null, source, inputMap, additionalData);
  }

  void import("@stillwind/host").then((host) => {
    try {
      const map = host.composeFrameworkSourceMap(
        inputMap,
        JSON.stringify(transform.map),
        transform.code,
      );
      callback(null, source, map, additionalData);
    } catch (error) {
      callback(error);
    }
  }, callback);
}
