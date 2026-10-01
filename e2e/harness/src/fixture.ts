/** Commands are resolved from the fixture's own dependencies. */
export type Driver =
  | "vite"
  | "nuxt"
  | "astro"
  | "next"
  | "react-router"
  | "webpack"
  | "rspack"
  | "rsbuild";

/** Expected behavior for one edit after its CSS has reached the browser. */
export interface Edit {
  file: string;
  from: string;
  to: string;
  target?: string;
  css: Record<string, string>;
  document: "preserve" | "reload";
  state: "preserve" | "reset" | "not-applicable";
}

/** A reason is required when a rendering contract does not apply. */
export type Capability = true | { status: "not-applicable"; reason: string };

/** Describes the application and the observable contracts shared by its profiles. */
export interface Fixture {
  id: string;
  driver: Driver;
  versions: string[];
  profiles: string[];
  page: {
    path: string;
    ready: string;
    target: string;
    css: Record<string, string>;
    styles?: { selector: string; css: Record<string, string> }[];
    counters: { selector: string; initial: string; clicked: string }[];
  };
  ssr: Capability;
  hydration: Capability;
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
  semantics?: { id: string; interactive: boolean }[];
  scenarios?: { name: string; script: string; directory: string; contracts: string[] }[];
  specialized?: { script: string; contracts: string[] };
}

/** Validates a manifest before any process starts or source file changes. */
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
    if (!Object.keys(edit.css).length) {
      throw new Error(`Edit ${name} requires a CSS expectation`);
    }
    if (!fixture.page.counters.length && edit.state !== "not-applicable") {
      throw new Error(`Edit ${name} requires a counter`);
    }
  }
  return fixture;
}

/** Selects a declared profile from the environment and rejects unknown profile names. */
export function fixtureProfiles(fixture: {
  id: string;
  profiles: readonly string[];
}): readonly string[] {
  const selected = process.env.STILLWIND_E2E_PROFILE;
  if (selected && !fixture.profiles.includes(selected)) {
    throw new Error(`Unknown profile ${selected} for ${fixture.id}`);
  }
  return selected ? [selected] : fixture.profiles;
}
