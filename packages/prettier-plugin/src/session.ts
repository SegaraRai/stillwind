import { SourceFormatter } from "@stillwind/analysis/node";
import type { RewriteInput, RewriteResult } from "@stillwind/analysis/node";

const sessions = new Map<
  string,
  {
    formatter: SourceFormatter;
    pending: number;
    timer?: ReturnType<typeof setTimeout>;
  }
>();

/** Reuses processes for nearby formatting calls and releases them after an idle interval. */
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
