/**
 * Tailwind spellings deliberately replaced by Stillwind's relationship-handle
 * API. Keep this list small: `--container-*` theme values and size utilities
 * remain ordinary Tailwind-compatible surface area.
 */
export const stillwindOwnedUtilityCandidates = new Set([
  "container",
  "@container",
  "@container-normal",
  "group",
  "peer",
]);

export function isStillwindOwnedUtilityCandidate(candidate: string) {
  return stillwindOwnedUtilityCandidates.has(candidate);
}
