/**
 * Runs candidate lookups with the WASM compiler. The worker loads the compiler and the recorded
 * default stylesheet once, then answers each `LookupRequest` with a `LookupReply` of the same id.
 * The compiler needs shared memory, so the page must be cross-origin isolated.
 */

import { lookupCandidates, type CandidateLookup } from "./lookup";
import { replayStylesheet, type RecordedStylesheet } from "./stylesheet";

/**
 * One lookup that the page requests.
 */
export interface LookupRequest {
  id: number;
  input: string;
}

/**
 * The answer to the request with the same id, or the reason why the compiler failed.
 */
export type LookupReply =
  | {
      id: number;
      lookup: CandidateLookup;
    }
  | {
      id: number;
      error: string;
    };

// The WASM binding awaits its module at the top level. A static import would delay the message
// handler until the compiler loads, and the worker would drop requests that arrive earlier.
const query = (async () => {
  const [{ NativeDesignSystemLoader }, response] = await Promise.all([
    import("@stillwind/native"),
    fetch(new URL("/docs/utilities-and-variants/stylesheet.json", import.meta.url)),
  ]);
  if (!response.ok) {
    throw new Error(`Stylesheet request failed: ${response.status}`);
  }
  const loader = new NativeDesignSystemLoader({});
  replayStylesheet(loader, (await response.json()) as RecordedStylesheet);
  return loader.editorQuery();
})();

self.onmessage = async ({ data }: MessageEvent<LookupRequest>) => {
  let reply: LookupReply;
  try {
    reply = {
      id: data.id,
      lookup: lookupCandidates(await query, data.input),
    };
  } catch (error) {
    reply = {
      id: data.id,
      error: error instanceof Error ? error.message : String(error),
    };
  }
  // The DOM library types `self` as a window. A worker scope posts like a `Worker` does.
  (self as unknown as Worker).postMessage(reply);
};
