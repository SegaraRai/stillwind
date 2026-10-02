interface NamedCase {
  name: string;
  suite?: NamedCase;
}

/** Returns the case and enclosing suite names for reports and artifact identifiers. */
export function fullCaseName(task: NamedCase): string {
  const names = [task.name];
  for (let suite = task.suite; suite; suite = suite.suite) {
    names.unshift(suite.name);
  }
  return names.join(" > ");
}
