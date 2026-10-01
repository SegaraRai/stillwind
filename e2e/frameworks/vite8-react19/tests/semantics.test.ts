import { fixtureProfiles } from "@stillwind/e2e-harness";
import { registerScript } from "@stillwind/e2e-harness/script";

import fixture from "../fixture.ts";

for (const profile of fixtureProfiles(fixture)) {
  for (const scenario of fixture.scenarios ?? []) {
    registerScript(
      scenario.name + "/" + profile,
      new URL("../", import.meta.url),
      scenario.script,
      [profile],
      scenario.directory,
    );
  }
}
