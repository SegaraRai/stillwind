import type { LintSeverity } from "@stillwind/config";

import { resolveRewriteConfig, type RewriteOptions } from "./rewrite-files.ts";
import { AnalysisWorker } from "./worker-client.ts";

/** Effective setting of an active lint rule. */
export type ActiveLintRule =
  | Exclude<LintSeverity, "off">
  | (Record<string, unknown> & {
      /** Active severity after preset composition and rule overrides. */
      severity: Exclude<LintSeverity, "off">;
    });

/** Resolved configuration summary for one project in a configuration file. */
export interface ProjectInfo {
  /** Zero-based project index in the evaluated configuration. */
  project: number;
  /** Optional configured project name. */
  name?: string;
  /** Absolute project root directory. */
  root: string;
  /** Absolute path of the configured stylesheet entry. */
  css: string;
  /** Configured target globs relative to root. */
  files: string[];
  /** Configured exclude globs relative to root. */
  exclude: string[];
  /** Number of source files currently matching the project's scope. */
  matchedFiles: number;
  /** Names of configured Stillwind plugins. */
  plugins: string[];
  /** Names of configured project analysis hooks. */
  hooks: string[];
  /** Whether utility sorting is enabled. */
  sort: boolean;
  /** Whether TypeScript type facts are enabled. */
  typeFacts: boolean;
  /** Active lint rules (excluding rules set to `off`), in rule-name order. */
  lint: Record<string, ActiveLintRule>;
}

/** Resolved project configurations for one configuration file. */
export interface InfoResult {
  /** Absolute path of the evaluated configuration. */
  config: string;
  /** Resolved projects in configuration order. */
  projects: ProjectInfo[];
}

/**
 * Resolves the configuration and returns each project's effective scope, plugins, and active lint
 * rules in a child process without compiling source files.
 *
 * @param options - Configuration discovery. Target globs do not apply.
 * @returns Configuration path and resolved project metadata.
 * @throws If configuration discovery, evaluation, worker initialization, or cleanup fails.
 */
export async function info(options: Omit<RewriteOptions, "patterns"> = {}): Promise<InfoResult> {
  const config = resolveRewriteConfig(options);
  const worker = new AnalysisWorker();
  try {
    await worker.open(config);
    const projects = await worker.request<ProjectInfo[]>({
      method: "info",
    });
    return {
      config,
      projects,
    };
  } finally {
    await worker.close();
  }
}
