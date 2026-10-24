import path from "node:path";

import type { BuildConfig, InlineConfig } from "@stillwind/config";

import { loaderOptionsId } from "./rules.ts";

/** Compiler options carried by a loader rule or an encoded SFC subrequest. */
export type LoaderOptions = InlineConfig & {
  /**
   * Base64url-encoded JSON configuration for an SFC subrequest. Omit when the rule supplies options
   * directly.
   */
  stillwindOptions?: string;
};

/** Producer identity and flags that select compilation behavior for one loader request. */
export interface ProducerRequest {
  /** Whether `stillwind.css` requests document CSS without importing application JavaScript. */
  projection: boolean;
  /** Whether `stillwind-module-css` requests an explicit CSS value module. */
  cssValue: boolean;
  /** Whether the resource is an Astro, Svelte, or Vue file, including its generated blocks. */
  sfc: boolean;
  /**
   * Full script identity. SFC blocks, projections, CSS values, and export selections share the
   * original producer identity.
   */
  id: string;
  /** Whether a `raw` or `url` query requires the loader to leave the input unchanged. */
  passthrough: boolean;
}

/**
 * Classifies a resource request and chooses the producer identity used for compilation.
 *
 * @param resourcePath - Absolute resource path without its query.
 * @param resourceQuery - Query including its leading `?`, or an empty string.
 * @returns Request flags and a producer ID. SFC blocks, CSS values, and projections share the
 * original file identity. Export-selection queries do not change the producer identity.
 */
export function classifyRequest(resourcePath: string, resourceQuery = ""): ProducerRequest {
  const projection = /[?&]stillwind\.css(?:&|$)/.test(resourceQuery);
  const cssValue = /[?&]stillwind-module-css(?:&|$)/.test(resourceQuery);
  const sfc = /\.(?:astro|svelte|vue)$/.test(resourcePath);
  const parameters = new URLSearchParams(resourceQuery);
  let sourceQuery = resourceQuery;
  if (parameters.has("export")) {
    parameters.delete("export");
    const query = parameters.toString();
    sourceQuery = query ? `?${query}` : "";
  }

  return {
    projection,
    cssValue,
    sfc,
    // SFC integrations re-enter the loader for blocks compiled from the original producer.
    id: resourcePath + (projection || cssValue || sfc ? "" : sourceQuery),
    passthrough: /[?&](?:raw|url)(?:[=&]|$)/.test(resourceQuery),
  };
}

/**
 * Reads compiler options from a rule or an encoded SFC subrequest.
 *
 * @param supplied - Rule options, optionally containing `stillwindOptions` encoded as base64url
 * JSON.
 * @returns Decoded options when `stillwindOptions` is present, otherwise the supplied object.
 * @throws If encoded configuration is not valid JSON.
 */
export function requestOptions(supplied: LoaderOptions): InlineConfig {
  return supplied.stillwindOptions
    ? JSON.parse(Buffer.from(supplied.stillwindOptions, "base64url").toString("utf8"))
    : supplied;
}

/**
 * Creates a module request with forward slashes, relative to the importer when possible.
 *
 * @param from - Absolute directory containing the importing module.
 * @param to - Absolute path of the requested resource.
 * @returns A relative request with `./` when needed, or an absolute path across Windows volumes.
 */
export function relativeRequest(from: string, to: string): string {
  const value = path.relative(from, to).replaceAll("\\", "/");
  return path.isAbsolute(value) || value.startsWith(".") ? value : `./${value}`;
}

/**
 * Creates a CSS projection or CSS value request that retains the producer configuration.
 *
 * SFC requests bypass other loaders. Naming callbacks use a registered rule identifier because JSON
 * cannot encode functions.
 *
 * @param resourcePath - Absolute path of the original producer.
 * @param sfc - Whether the producer is an Astro, Svelte, or Vue file.
 * @param config - Compiler config used for the original producer.
 * @param loaderPath - Absolute path of the producer loader.
 * @param query - Stillwind query text without a leading `?`.
 * @param identityConfig - Original inline settings registered on the loader rule. When evaluated
 * config contains naming callbacks, this selects that rule instead of serializing callbacks.
 * Defaults to config when file evaluation has not changed the registered settings.
 * @returns A relative resource request for scripts, or an inline loader request for SFCs.
 * @throws If SFC settings cannot be serialized for their inline request.
 */
export function virtualRequest(
  resourcePath: string,
  sfc: boolean,
  config: BuildConfig,
  loaderPath: string,
  query: string,
  identityConfig: InlineConfig = config,
): string {
  if (!sfc) {
    return `${relativeRequest(path.dirname(resourcePath), resourcePath)}?${query}`;
  }

  const configuration = config.naming
    ? "??" + loaderOptionsId(identityConfig)
    : "?stillwindOptions=" + Buffer.from(JSON.stringify(config)).toString("base64url");
  return `!!${loaderPath.replaceAll("\\", "/")}${configuration}!${resourcePath.replaceAll("\\", "/")}?${query}`;
}
