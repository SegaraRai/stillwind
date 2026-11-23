/**
 * In-memory formatting hooks for CLI `fmt` and `check` commands.
 */

/** Command that invokes a project hook. */
export type HookCommand = "fmt" | "check";

/** Whether the invoking command writes changed files or only checks them. */
export type HookMode = "write" | "check";

/** Execution context shared across a hook's lifecycle and callbacks. */
export interface HookContext {
  /** Normalized absolute project root directory. */
  readonly root: string;
  /** Optional configured project name. */
  readonly project?: string;
  /** CLI command that started this run. */
  readonly command: HookCommand;
  /** `"write"` saves modified buffers; `"check"` compares buffers without saving. */
  readonly mode: HookMode;
  /** Signal aborted if the caller cancels the run. */
  readonly signal: AbortSignal;
}

/** One target buffer passed to {@link AnalysisHook.format}. */
export interface FormatHookFile {
  /** Absolute path of the target file. */
  readonly path: string;
  /** Current text after Stillwind sorting or fixing and any earlier format hooks. */
  readonly code: string;
  /** Original text read from disk before Stillwind and hooks ran. */
  readonly original: string;
  /** Whether Stillwind or an earlier hook has already changed this buffer from `original`. */
  readonly changed: boolean;
}

/**
 * Project-scoped hook that transforms rewritten buffers in memory during CLI `fmt` and `check`
 * runs.
 */
export interface AnalysisHook {
  /** Nonempty identifier shown in `stillwind info` and failure messages. */
  readonly name: string;
  /**
   * Initializes resources once per project before any `format` callbacks run.
   *
   * @param context - Project identity and command mode for this run.
   */
  setup?(context: HookContext): void | Promise<void>;
  /**
   * Releases resources after `format` callbacks finish. Runs only when `setup` succeeded.
   *
   * @param context - Project identity and command mode for this run.
   */
  teardown?(context: HookContext): void | Promise<void>;
  /**
   * Transforms one target buffer in memory during `fmt` and `check` commands.
   *
   * @param file - Current and original buffer text for one target.
   * @param context - Project identity and command mode for this run.
   * @returns Replacement text, or `undefined` to leave the buffer unchanged.
   */
  format?(file: FormatHookFile, context: HookContext): string | void | Promise<string | void>;
}

/**
 * Returns `hook` unchanged with its type checked, for use in a configuration file.
 *
 * @param hook - Analysis hook definition.
 * @returns The same object.
 */
export function defineHook(hook: AnalysisHook): AnalysisHook {
  return hook;
}
