import type { ExpectedFailure } from "./fixture.ts";

/** Priorities describe failures of observable contracts, independently of the fixture's support tier. */
export const contractPolicy = {
  cssRendering: { priority: "high", description: "Rendered classes have matching computed CSS." },
  sourceUpdate: {
    priority: "high",
    description: "Edits update CSS with the framework's ordinary reload and state behavior.",
  },
  serverRendering: {
    priority: "high",
    description: "Fresh server HTML receives current CSS and agrees with client classes.",
  },
  interactions: {
    priority: "high",
    description: "Hydration and component interactions remain functional.",
  },
  navigation: { priority: "high", description: "Navigation loads the destination's styles." },
  semantics: {
    priority: "high",
    description: "Composition and conditional styles preserve their behavior.",
  },
  buildInvalidation: {
    priority: "high",
    description: "Rebuilds and restored producers emit current CSS.",
  },
  sourceMaps: {
    priority: "low",
    description: "Generated classes map to their original macro positions.",
  },
} as const;

export type ContractId = keyof typeof contractPolicy;
export interface CasePolicy {
  contract: ContractId;
  expectedFailure?: ExpectedFailure;
}

/** Adds the contract's priority to a case's declared policy for reports. */
export function casePolicy(policy: CasePolicy) {
  return { ...policy, priority: contractPolicy[policy.contract].priority };
}
