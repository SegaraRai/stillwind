import { registerFixture } from "@stillwind-internal/e2e-harness/acceptance";

import fixture from "../fixture.mts";

registerFixture(fixture, new URL("../", import.meta.url));
