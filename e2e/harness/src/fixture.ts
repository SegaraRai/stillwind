export type SupportTier = "primary" | "compatibility";

/** A profile override describes a different host or integration within the same application. */
export interface Support {
  tier: SupportTier;
  profiles?: Record<string, SupportTier>;
}

export interface FixtureManifest {
  id: string;
  profiles: readonly string[];
  support: Support;
}

export interface ExpectedFailure {
  reason: string;
  issue?: string;
}

/**
 * Declares that a fixture cannot build on the listed values of `process.platform`.
 * These platforms run one expected-failure build case instead of the fixture contracts.
 */
export interface PlatformFailure extends ExpectedFailure {
  platforms: NodeJS.Platform[];
}

/** Returns the declared support tier for a profile; rejects unknown profiles and tiers. */
export function fixtureSupport(fixture: FixtureManifest, profile: string): SupportTier {
  if (!fixture.profiles.includes(profile)) {
    throw new Error(`Unknown profile ${profile} for ${fixture.id}`);
  }
  for (const name of Object.keys(fixture.support.profiles ?? {})) {
    if (!fixture.profiles.includes(name)) {
      throw new Error(`Unknown support profile ${name} for ${fixture.id}`);
    }
  }
  const tier = fixture.support.profiles?.[profile] ?? fixture.support.tier;
  if (tier !== "primary" && tier !== "compatibility") {
    throw new Error(`Invalid support tier: ${String(tier)}`);
  }
  return tier;
}

/** Validates a known limitation and returns its explanation for case names. */
export function failureReason(failure: ExpectedFailure): string {
  if (!failure.reason.trim()) {
    throw new Error("An expected failure requires a reason");
  }
  if (failure.issue !== undefined && !failure.issue.trim()) {
    throw new Error("An expected failure issue must not be empty");
  }
  return failure.reason;
}

/**
 * Commands are resolved from the fixture's own dependencies.
 */
export type Driver =
  | "vite"
  | "nuxt"
  | "astro"
  | "fresh"
  | "gatsby"
  | "next"
  | "react-router"
  | "webpack"
  | "rspack"
  | "rsbuild";

/**
 * Expected behavior for one edit after its CSS has reached the browser.
 */
export interface Edit {
  file: string;
  from: string;
  to: string;
  target?: string;
  css: Record<string, string>;
  document: "preserve" | "reload";
  state: "preserve" | "reset" | "not-applicable";
  counters?: string[];
  expectedFailure?: ExpectedFailure;
}

/**
 * A reason is required when a rendering contract does not apply.
 */
export type Capability = true | { status: "not-applicable"; reason: string };

/**
 * Describes the application and the observable contracts shared by its profiles.
 */
export interface Fixture extends FixtureManifest {
  id: string;
  driver: Driver;
  versions: string[];
  profiles: string[];
  page: {
    path: string;
    ready: string;
    target: string;
    css: Record<string, string>;
    styles?: {
      selector: string;
      css: Record<string, string>;
    }[];
    counters: {
      selector: string;
      initial: string;
      clicked: string;
    }[];
    updates?: {
      trigger: string;
      target: string;
      css: Record<string, string>;
    }[];
  };
  ssr: Capability;
  hydration: Capability;
  shadowDom?: {
    hosts: [string, string];
    target: string;
    outside: string;
  };
  prerender?: Capability;
  edits: Record<string, Edit>;
  navigation?: {
    link: string;
    target: string;
    css: Record<string, string>;
    back: string;
    document: "preserve" | "reload";
  };
  production?: boolean;
  platformFailure?: PlatformFailure;
  cachedProducer?: string;
  sourceMaps?: {
    file: string;
    macro: string;
    kind: "javascript" | "css";
    development?: {
      expectedFailure?: ExpectedFailure;
    };
    expectedFailure?: ExpectedFailure | Record<string, ExpectedFailure>;
  }[];
  semantics?: {
    id: string;
    interactive: boolean;
  }[];
  scenarios?: {
    name: string;
    directory: string;
    contracts: string[];
  }[];
  specialized?: {
    contracts: string[];
  };
}

/**
 * Validates a manifest before any process starts or source file changes.
 */
export function defineFixture(fixture: Fixture): Fixture {
  if (!/^[a-z0-9][a-z0-9-]*$/.test(fixture.id)) {
    throw new Error(`Invalid fixture id: ${fixture.id}`);
  }
  if (!fixture.profiles.length || new Set(fixture.profiles).size !== fixture.profiles.length) {
    throw new Error("Profiles must be nonempty and unique");
  }
  for (const capability of [fixture.ssr, fixture.hydration, fixture.prerender]) {
    if (capability && capability !== true && !capability.reason.trim()) {
      throw new Error("An inapplicable contract requires a reason");
    }
  }
  if (fixture.hydration === true && fixture.ssr !== true && fixture.prerender !== true) {
    throw new Error("Hydration requires server-rendered or prerendered HTML");
  }
  for (const [name, edit] of Object.entries(fixture.edits)) {
    if (!edit.from || edit.from === edit.to) {
      throw new Error(`Edit ${name} must change a nonempty literal`);
    }
    if (edit.expectedFailure) {
      failureReason(edit.expectedFailure);
    }
    if (!Object.keys(edit.css).length) {
      throw new Error(`Edit ${name} requires a CSS expectation`);
    }
    if (!fixture.page.counters.length && edit.state !== "not-applicable") {
      throw new Error(`Edit ${name} requires a counter`);
    }
  }
  if (fixture.platformFailure) {
    failureReason(fixture.platformFailure);
    if (!fixture.platformFailure.platforms.length) {
      throw new Error("A platform failure requires a platform");
    }
  }
  for (const mapping of fixture.sourceMaps ?? []) {
    if (mapping.development && mapping.kind !== "javascript") {
      throw new Error("Development source map checks require a JavaScript mapping");
    }
    if (mapping.development?.expectedFailure) {
      failureReason(mapping.development.expectedFailure);
    }
    if (mapping.expectedFailure) {
      if ("reason" in mapping.expectedFailure) {
        failureReason(mapping.expectedFailure as ExpectedFailure);
      } else {
        for (const [profile, failure] of Object.entries(mapping.expectedFailure)) {
          if (!fixture.profiles.includes(profile)) {
            throw new Error(`Unknown failure profile ${profile}`);
          }
          failureReason(failure);
        }
      }
    }
  }
  for (const profile of fixture.profiles) {
    fixtureSupport(fixture, profile);
  }
  return fixture;
}

/**
 * Selects a declared profile from the environment and rejects unknown profile names.
 */
export function fixtureProfiles(fixture: FixtureManifest): readonly string[] {
  const selected = process.env.STILLWIND_E2E_PROFILE;
  if (selected && !fixture.profiles.includes(selected)) {
    throw new Error(`Unknown profile ${selected} for ${fixture.id}`);
  }
  const tier = process.env.STILLWIND_E2E_SUPPORT;
  if (tier && tier !== "primary" && tier !== "compatibility") {
    throw new Error(`Invalid support tier: ${String(tier)}`);
  }
  return (selected ? [selected] : fixture.profiles).filter(
    (profile) => !tier || fixtureSupport(fixture, profile) === tier,
  );
}
