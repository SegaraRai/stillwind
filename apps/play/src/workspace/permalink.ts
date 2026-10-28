/**
 * Workspace state that a permalink restores.
 */
export interface PlaygroundState {
  /** Preset that the workspace started from. */
  preset: string;
  /** Stillwind version of the site that created the link. */
  stillwind: string;
  /** Complete text of every editable file, keyed by path relative to `/workspace/`. */
  files: Record<string, string>;
  /** Path of the file open in the editor, relative to `/workspace/`. */
  active?: string;
}

/**
 * Location fragment that selects how the playground starts.
 */
export type PlaygroundLocation =
  | {
      /** `#preset=<id>` starts from a preset. */
      kind: "preset";
      id: string;
    }
  | {
      /** `#code=<data>` restores a shared workspace. */
      kind: "permalink";
      data: string;
    };

/**
 * Reads the location fragment.
 *
 * @param hash - `location.hash`, with or without the leading `#`.
 * @returns The requested start, or `undefined` for an empty or unknown fragment.
 */
export function readLocation(hash: string): PlaygroundLocation | undefined {
  const params = new URLSearchParams(hash.replace(/^#/, ""));
  const data = params.get("code");
  if (data) {
    return {
      kind: "permalink",
      data,
    };
  }
  const id = params.get("preset");
  return id
    ? {
        kind: "preset",
        id,
      }
    : undefined;
}

/**
 * Encodes workspace state for a `#code=` fragment. Each link contains the complete files, so it
 * reproduces the workspace even after its preset changes.
 *
 * @returns Base64url text of the deflate-compressed JSON state.
 */
export async function encodeState(state: PlaygroundState): Promise<string> {
  const json = new TextEncoder().encode(
    JSON.stringify({
      version: 1,
      ...state,
    }),
  );
  const bytes = await transform(json, new CompressionStream("deflate-raw"));
  let binary = "";
  for (const byte of bytes) {
    binary += String.fromCharCode(byte);
  }
  return btoa(binary).replaceAll("+", "-").replaceAll("/", "_").replace(/=+$/, "");
}

/**
 * Decodes the data of a `#code=` fragment.
 *
 * @throws If the data is not a version 1 state that `encodeState` produced.
 */
export async function decodeState(data: string): Promise<PlaygroundState> {
  let value: unknown;
  try {
    const binary = atob(data.replaceAll("-", "+").replaceAll("_", "/"));
    const bytes = Uint8Array.from(binary, (char) => char.charCodeAt(0));
    value = JSON.parse(
      new TextDecoder().decode(await transform(bytes, new DecompressionStream("deflate-raw"))),
    );
  } catch {
    value = undefined;
  }
  if (
    !value ||
    typeof value !== "object" ||
    !("version" in value) ||
    value.version !== 1 ||
    !("preset" in value) ||
    typeof value.preset !== "string" ||
    !("stillwind" in value) ||
    typeof value.stillwind !== "string" ||
    !("files" in value) ||
    !value.files ||
    typeof value.files !== "object" ||
    !Object.values(value.files).every((text) => typeof text === "string") ||
    ("active" in value && typeof value.active !== "string")
  ) {
    throw new Error("The link does not contain a playground workspace");
  }
  const { preset, stillwind, files, active } = value as PlaygroundState;
  return {
    preset,
    stillwind,
    files,
    active,
  };
}

async function transform(
  bytes: Uint8Array<ArrayBuffer>,
  stream: CompressionStream | DecompressionStream,
): Promise<Uint8Array<ArrayBuffer>> {
  const output = new Response(new Blob([bytes]).stream().pipeThrough(stream));
  return new Uint8Array(await output.arrayBuffer());
}
