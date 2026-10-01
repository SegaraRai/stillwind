import { fixtureProfiles } from "@stillwind/e2e-harness";
import { registerScript } from "@stillwind/e2e-harness/script";

import fixture from "../fixture.ts";

for (const profile of fixtureProfiles(fixture)) {
  registerScript(
    fixture.id + "-" + profile,
    new URL("../", import.meta.url),
    fixture.specialized!.script,
    [profile],
  );
}
