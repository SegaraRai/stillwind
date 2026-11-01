import type { DesignCandidate, DesignSystemRequest, DesignToken } from "@stillwind/host";

import { resolveRewriteConfig, type RewriteOptions } from "./rewrite-files.ts";
import { AnalysisWorker } from "./worker-client.ts";

/**
 * Design-system answers from every project of one configuration.
 */
export interface DesignSystemAnswer {
  /** Absolute path of the evaluated configuration. */
  config: string;
  /** Tokens in project and name order. */
  tokens: (DesignToken & {
    /** Zero-based project index in the evaluated configuration. */
    project: number;
  })[];
  /** Candidates in project order. */
  candidates: (DesignCandidate & {
    /** Zero-based project index in the evaluated configuration. */
    project: number;
  })[];
  /** Targets without a match in any project. */
  unmatched: string[];
}

/**
 * Answers a question about each project's design system in a child process.
 *
 * @param options - Configuration discovery. Target globs do not apply.
 * @param request - Theme, utility, or variant request.
 * @returns Answers from every project. Unknown names and unmatched targets are data.
 * @throws If configuration discovery, worker initialization, CSS loading, a plugin, or cleanup fails.
 */
export async function designSystem(
  options: Omit<RewriteOptions, "patterns">,
  request: DesignSystemRequest,
): Promise<DesignSystemAnswer> {
  const config = resolveRewriteConfig(options);
  const worker = new AnalysisWorker();
  try {
    await worker.open(config);
    const answers = await worker.request<
      {
        project: number;
        result: {
          tokens: DesignToken[];
          candidates: DesignCandidate[];
          unmatched: string[];
        };
      }[]
    >({
      method: "designSystem",
      request,
    });
    return {
      config,
      tokens: answers.flatMap(({ project, result }) =>
        result.tokens.map((token) => ({
          ...token,
          project,
        })),
      ),
      candidates: answers.flatMap(({ project, result }) =>
        result.candidates.map((candidate) => ({
          ...candidate,
          project,
        })),
      ),
      unmatched: request.targets.filter((target) =>
        answers.every(({ result }) => result.unmatched.includes(target)),
      ),
    };
  } finally {
    await worker.close();
  }
}
