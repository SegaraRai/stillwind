/**
 * Publishes elapsed times of host compilation phases for performance measurement.
 *
 * Messages go to the `stillwind:host:phase` channel of `node:diagnostics_channel`.
 * Publishing does nothing when the runtime has no such module, such as a browser,
 * or when the channel has no subscribers.
 */

/** One completed phase. */
export interface PhaseMessage {
  /** Phase name, such as `scan` or `native-compile`. */
  phase: string;
  /**
   * Milliseconds from start to completion. An asynchronous phase also counts the time
   * that it waits for other work, so durations of concurrent phases can overlap.
   */
  duration: number;
  /** Number of items that the phase processed, such as sources passed to one call. */
  items: number;
}

const channel = (
  (
    globalThis as {
      process?: {
        getBuiltinModule?: (id: string) => unknown;
      };
    }
  ).process?.getBuiltinModule?.("node:diagnostics_channel") as
    | typeof import("node:diagnostics_channel")
    | undefined
)?.channel("stillwind:host:phase");

const ignore = () => {};

/**
 * Starts a phase measurement.
 *
 * @param phase - Phase name to publish.
 * @returns A function that publishes the phase with its item count, which defaults to 1.
 * The function does nothing when no subscriber was present at the start.
 */
export function startPhase(phase: string): (items?: number) => void {
  if (!channel?.hasSubscribers) {
    return ignore;
  }
  const start = performance.now();
  return (items = 1) => {
    channel.publish({
      phase,
      duration: performance.now() - start,
      items,
    } satisfies PhaseMessage);
  };
}

/**
 * Measures a synchronous or asynchronous operation as one phase.
 *
 * @param phase - Phase name to publish.
 * @param items - Item count to publish.
 * @param run - Operation to measure. A returned promise ends the phase when it settles.
 * @returns The result of `run`. Failures are rethrown after the phase is published.
 */
export function tracePhase<T>(phase: string, items: number, run: () => T): T {
  const end = startPhase(phase);
  if (end === ignore) {
    return run();
  }
  let result: T;
  try {
    result = run();
  } catch (error) {
    end(items);
    throw error;
  }
  if (result instanceof Promise) {
    return result.finally(() => end(items)) as T;
  }
  end(items);
  return result;
}
