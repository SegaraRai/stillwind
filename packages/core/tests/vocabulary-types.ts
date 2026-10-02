import type { StillwindUtility, StillwindVariantVocabulary } from "stillwind/macro";

declare module "stillwind/macro" {
  interface StillwindUtilityVocabulary {
    "shared-test-utility": true;
  }
  interface StillwindVariantVocabulary {
    "shared-test-variant": true;
  }
}

type Assert<T extends true> = T;
type Same<A, B> = [A, B] extends [B, A] ? true : false;
export type SharedVocabularyContracts = [
  Assert<Same<StillwindUtility, "shared-test-utility" | `shared-test-variant:${string}`>>,
  Assert<Same<StillwindUtility, import("stillwind/react").StillwindUtility>>,
  Assert<Same<StillwindUtility, import("stillwind/preact").StillwindUtility>>,
  Assert<Same<StillwindUtility, import("stillwind/solid").StillwindUtility>>,
  Assert<Same<StillwindUtility, import("stillwind/vue").StillwindUtility>>,
  Assert<Same<StillwindUtility, import("stillwind/svelte").StillwindUtility>>,
  Assert<Same<StillwindUtility, import("stillwind/astro").StillwindUtility>>,
  Assert<Same<keyof StillwindVariantVocabulary, "shared-test-variant">>,
];

export type FrameworkResultContracts = [
  Assert<Same<import("stillwind/react").StyleResult<"r">["className"], string | undefined>>,
  Assert<Same<import("stillwind/preact").StyleResult<"p">["class"], string | undefined>>,
  Assert<
    Same<import("stillwind/solid").StyleResult<"s">["style"], Record<string, string> | undefined>
  >,
  Assert<
    Same<import("stillwind/vue").StyleResult<"v">["style"], Record<string, string> | undefined>
  >,
  Assert<Same<import("stillwind/svelte").StyleResult<"s">["style"], string | undefined>>,
  Assert<Same<import("stillwind/astro").StyleResult<"a">["style"], string | undefined>>,
];

type SharedRefExports<Framework> = Same<Framework, import("stillwind/macro").RefVariant<"r">>;

export type FrameworkRefContracts = [
  Assert<SharedRefExports<import("stillwind/react").RefVariant<"r">>>,
  Assert<SharedRefExports<import("stillwind/preact").RefVariant<"r">>>,
  Assert<SharedRefExports<import("stillwind/solid").RefVariant<"r">>>,
  Assert<SharedRefExports<import("stillwind/vue").RefVariant<"r">>>,
  Assert<SharedRefExports<import("stillwind/svelte").RefVariant<"r">>>,
  Assert<SharedRefExports<import("stillwind/astro").RefVariant<"r">>>,
  Assert<
    Same<
      import("stillwind/react").ScopedRefVariant<"r", readonly ["s"]>,
      import("stillwind/macro").ScopedRefVariant<"r", readonly ["s"]>
    >
  >,
  Assert<
    Same<
      import("stillwind/astro").ContainerVariant<"r">,
      import("stillwind/macro").ContainerVariant<"r">
    >
  >,
];
