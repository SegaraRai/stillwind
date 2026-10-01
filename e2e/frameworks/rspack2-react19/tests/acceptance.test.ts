import { registerFixture } from "@stillwind/e2e-harness/acceptance";

import fixture from "../fixture.ts";

registerFixture(fixture, new URL("../", import.meta.url));
