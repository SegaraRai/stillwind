import { normalizeWorkspacePath } from "./path.ts";
import { decodeState, readLocation, type PlaygroundState } from "./permalink.ts";
import { presets, type Preset } from "./presets.ts";

/**
 * Workspace that the playground opens.
 */
export interface Start {
  /** Preset that the workspace starts from. A permalink keeps the preset that created it. */
  preset: Preset;
  /** Editable files keyed by absolute virtual path. */
  files: Map<string, string>;
  /** Absolute virtual path of the file to show first. */
  active: string;
  /** Explains a fallback or a version difference to the user. */
  notice?: string;
}

const ENTRY = "/workspace/App.tsx";

/**
 * Selects the workspace from the location fragment. An empty fragment opens the first preset.
 * Unknown presets and unreadable permalinks also open the first preset and set a notice.
 *
 * @param hash - `location.hash`.
 */
export async function resolveStart(hash: string): Promise<Start> {
  const location = readLocation(hash);
  if (location?.kind === "preset") {
    const preset = presets.find(({ id }) => id === location.id);
    return preset
      ? fromPreset(preset)
      : {
          ...fromPreset(presets[0]),
          notice: `The preset "${location.id}" does not exist.`,
        };
  }
  if (location?.kind === "permalink") {
    let state: PlaygroundState;
    try {
      state = await decodeState(location.data);
    } catch (error) {
      return {
        ...fromPreset(presets[0]),
        notice: `The link cannot be opened: ${error instanceof Error ? error.message : String(error)}`,
      };
    }
    // Entries whose paths leave the workspace or are empty are ignored.
    const files = new Map<string, string>();
    for (const [path, text] of Object.entries(state.files)) {
      const normalized = normalizeWorkspacePath(path);
      if (normalized) {
        files.set(`/workspace/${normalized}`, text);
      }
    }
    if (!files.has(ENTRY)) {
      return {
        ...fromPreset(presets[0]),
        notice: "The link does not contain App.tsx.",
      };
    }
    const active = state.active === undefined ? null : normalizeWorkspacePath(state.active);
    return {
      preset: presets.find(({ id }) => id === state.preset) ?? presets[0],
      files,
      active: active !== null && files.has(`/workspace/${active}`) ? `/workspace/${active}` : ENTRY,
      notice:
        state.stillwind === __STILLWIND_VERSION__
          ? undefined
          : `This link was created with Stillwind ${state.stillwind}. This site runs Stillwind ${__STILLWIND_VERSION__}, so results can differ.`,
    };
  }
  return fromPreset(presets[0]);
}

/**
 * Captures the workspace for a permalink.
 *
 * @param files - Editable files keyed by absolute virtual path.
 * @param active - Absolute virtual path of the file open in the editor.
 */
export function captureState(
  preset: Preset,
  files: ReadonlyMap<string, string>,
  active: string,
): PlaygroundState {
  return {
    preset: preset.id,
    stillwind: __STILLWIND_VERSION__,
    files: Object.fromEntries(
      [...files].map(([path, text]) => [path.replace("/workspace/", ""), text]),
    ),
    active: active.replace("/workspace/", ""),
  };
}

function fromPreset(preset: Preset): Start {
  return {
    preset,
    files: new Map(preset.files),
    active: ENTRY,
  };
}
