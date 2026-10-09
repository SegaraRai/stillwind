import { SourceFormatter, type RewriteInput, type RewriteResult } from "@stillwind/analysis/node";

/** Formatters shared by the exact configuration path across calls in this Node process. */
const sessions = new Map<
  string,
  {
    /** Isolated analysis process with serialized formatting requests. */
    formatter: SourceFormatter;
    /** Number of calls that have not settled, including calls waiting in the formatter queue. */
    pending: number;
    /** Unreferenced 250 ms timer that releases the idle process after the last call settles. */
    timer?: ReturnType<typeof setTimeout>;
  }
>();

/**
 * Reuses a formatter for the configuration and releases it after 250 ms with no pending calls.
 *
 * @param config - Absolute configuration path used as the process-cache key.
 * @param input - Unsaved original buffer and protected sorting ranges, in original UTF-16 coordinates.
 * @returns Sorting output without writing the file. The pending count is restored on success or failure.
 * @throws If the formatter cannot load the project or compute a valid rewrite.
 */
export async function sortBuffer(config: string, input: RewriteInput): Promise<RewriteResult> {
  let session = sessions.get(config);
  if (!session) {
    session = {
      formatter: new SourceFormatter(config),
      pending: 0,
    };
    sessions.set(config, session);
  }
  clearTimeout(session.timer);
  session.pending++;
  try {
    return await session.formatter.format(input);
  } finally {
    session.pending--;
    if (!session.pending) {
      const current = session;
      session.timer = setTimeout(() => {
        sessions.delete(config);
        void current.formatter
          .close()
          .catch((error: unknown) => process.stderr.write(String(error) + "\n"));
      }, 250);
      session.timer.unref();
    }
  }
}
