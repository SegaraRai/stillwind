import { availableParallelism } from "node:os";

import { AnalysisWorker } from "./worker-client.ts";

/**
 * Splits configured projects into groups that separate worker processes analyze concurrently.
 *
 * Projects that share a target stay in one group, because a rewrite must compare the answers of
 * every project that includes the target. Groups are balanced by their number of distinct targets.
 *
 * @param targets - Absolute target paths of each project, in project order. Projects without
 * targets are still assigned to a group.
 * @param limit - Maximum number of groups. Values below 1 are treated as 1.
 * @returns Non-empty groups of ascending zero-based project indices, ordered by their first index.
 * Every project index appears in exactly one group. An empty input returns no groups.
 */
export function groupProjects(targets: readonly (readonly string[])[], limit: number): number[][] {
  const parents = targets.map((_, index) => index);
  const root = (index: number): number => {
    while (parents[index] !== index) {
      parents[index] = parents[parents[index]];
      index = parents[index];
    }
    return index;
  };
  const owners = new Map<string, number>();
  for (const [index, files] of targets.entries()) {
    for (const file of files) {
      const owner = owners.get(file);
      if (owner === undefined) {
        owners.set(file, index);
      } else {
        parents[root(index)] = root(owner);
      }
    }
  }
  const components = new Map<number, { projects: number[]; files: Set<string> }>();
  for (const [index, files] of targets.entries()) {
    const component = components.get(root(index)) ?? { projects: [], files: new Set<string>() };
    component.projects.push(index);
    for (const file of files) {
      component.files.add(file);
    }
    components.set(root(index), component);
  }
  // Longest-processing-time assignment: place the largest component in the lightest group.
  const bins = Array.from(
    { length: Math.min(components.size, Math.max(1, Math.floor(limit))) },
    () => ({
      projects: [] as number[],
      weight: 0,
    }),
  );
  const ordered = [...components.values()].sort(
    (a, b) => b.files.size - a.files.size || a.projects[0] - b.projects[0],
  );
  for (const component of ordered) {
    const bin = bins.reduce((lightest, candidate) =>
      candidate.weight < lightest.weight ? candidate : lightest,
    );
    bin.projects.push(...component.projects);
    bin.weight += component.files.size;
  }
  return bins
    .map((bin) => bin.projects.sort((a, b) => a - b))
    .filter((projects) => projects.length)
    .sort((a, b) => a[0] - b[0]);
}

/**
 * Resolves the maximum number of analysis worker processes.
 *
 * @param concurrency - Requested maximum. Omission selects half of the available parallelism.
 * @returns A positive integer.
 * @throws If concurrency is present and is not a positive integer.
 */
export function analysisConcurrency(concurrency?: number): number {
  if (concurrency === undefined) {
    return Math.max(1, Math.floor(availableParallelism() / 2));
  }
  if (!Number.isInteger(concurrency) || concurrency < 1) {
    throw new Error(`Analysis concurrency must be a positive integer: ${concurrency}`);
  }
  return concurrency;
}

/** One worker process and the configured targets that it analyzes. */
export interface ProjectWorkerGroup {
  /** Worker with the complete configuration open. Result project indices are configuration indices. */
  worker: AnalysisWorker;
  /**
   * Zero-based indices of the projects that this worker analyzes, in ascending order. Undefined
   * means every project, when one worker handles the whole configuration.
   */
  projects?: number[];
  /** Sorted, distinct absolute targets of these projects. No other group includes them. */
  files: string[];
}

/**
 * Worker processes that analyze disjoint project groups of one configuration concurrently.
 * Close it after use, also when an operation fails.
 */
export class ProjectWorkers {
  /** Every started worker, including workers that no group uses. */
  readonly #workers: AnalysisWorker[];
  /** Groups in order of their first project index. */
  readonly groups: readonly ProjectWorkerGroup[];
  /** Sorted, distinct absolute targets of every group. */
  readonly files: readonly string[];

  private constructor(workers: AnalysisWorker[], groups: ProjectWorkerGroup[]) {
    this.#workers = workers;
    this.groups = groups;
    this.files = [...new Set(groups.flatMap((group) => group.files))].sort();
  }

  /**
   * Opens the configuration in one worker, enumerates targets, and opens more workers when the
   * configuration has several project groups.
   *
   * @param config - Absolute configuration path.
   * @param options - Target filters, stylesheet selection, and the maximum number of workers.
   * @returns Workers whose groups partition the selected targets. A group can have no files.
   * @throws If a worker cannot open the configuration, enumeration fails, or concurrency is invalid.
   * Started workers are closed before the error is rethrown.
   */
  static async open(
    config: string,
    options: {
      /** Additional project-relative globs. Omission or an empty list keeps every configured target. */
      patterns?: readonly string[];
      /** Adds the stylesheets that each project lints to the targets. */
      stylesheets?: boolean;
      /** Maximum number of worker processes. Omission selects half of the available parallelism. */
      concurrency?: number;
    },
  ): Promise<ProjectWorkers> {
    const limit = analysisConcurrency(options.concurrency);
    const first = new AnalysisWorker();
    const workers = [first];
    try {
      const count = await first.open(config);
      if (Math.min(count, limit) <= 1) {
        const files = await first.targets(options.patterns, options.stylesheets);
        return new ProjectWorkers(workers, [{ worker: first, files }]);
      }
      // Additional workers evaluate the configuration while the first worker enumerates targets.
      const extra = Array.from({ length: Math.min(count, limit) - 1 }, () => new AnalysisWorker());
      workers.push(...extra);
      const opening = Promise.all(extra.map((worker) => worker.open(config)));
      const [targets] = await Promise.all([
        first.projectTargets(options.patterns, options.stylesheets),
        opening,
      ]);
      const groups = groupProjects(targets, limit).map((projects, index) => ({
        worker: workers[index],
        projects,
        files: [...new Set(projects.flatMap((project) => targets[project]))].sort(),
      }));
      const unused = workers.splice(groups.length);
      await Promise.all(unused.map((worker) => worker.close()));
      return new ProjectWorkers(workers, groups);
    } catch (error) {
      await Promise.allSettled(workers.map((worker) => worker.close()));
      throw error;
    }
  }

  /**
   * Closes every worker and waits for each process to exit.
   *
   * @throws The first cleanup failure after every worker has been closed.
   */
  async close(): Promise<void> {
    const results = await Promise.allSettled(this.#workers.map((worker) => worker.close()));
    const failure = results.find((result) => result.status === "rejected");
    if (failure) {
      throw failure.reason;
    }
  }
}
