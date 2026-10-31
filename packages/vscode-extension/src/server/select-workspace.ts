/**
 * Selects the workspace of an operation that uses one workspace.
 *
 * @param workspaces - Currently opened workspaces, keyed by configuration identity.
 * @param failures - Error messages of configurations that failed to open, keyed by identity.
 * @param operation - Name of the operation for error messages.
 * @param config - Configuration identity of the requested workspace. Omission requires exactly one
 * opened workspace.
 * @returns The configuration identity and the workspace.
 * @throws If the requested workspace is not open, or omission finds no workspace or several
 * workspaces. The error contains the failure of the requested configuration, or all failures when
 * no workspace is open.
 */
export function selectWorkspace<T>(
  workspaces: ReadonlyMap<string, T>,
  failures: ReadonlyMap<string, string>,
  operation: string,
  config?: string,
): [string, T] {
  if (config !== undefined) {
    const workspace = workspaces.get(config);
    if (workspace) {
      return [config, workspace];
    }
    const failure = failures.get(config);
    throw new Error(
      failure === undefined
        ? `${operation} cannot find the Stillwind workspace ${config}`
        : `${config}: ${failure}`,
    );
  }
  if (workspaces.size === 1) {
    return [...workspaces][0];
  }
  if (workspaces.size) {
    throw new Error(
      `${operation} accepts one Stillwind workspace, but ${workspaces.size} are open`,
    );
  }
  throw new Error(
    failures.size
      ? [...failures].map(([id, error]) => `${id}: ${error}`).join("\n")
      : `${operation} requires a Stillwind configuration`,
  );
}
