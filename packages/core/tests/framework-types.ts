import { style as astro } from "stillwind/astro";
import type { ContainerVariant, RefVariant, ScopedRefVariant, StyleRef } from "stillwind/macro";
import css from "stillwind/module-css";
import { style as preact } from "stillwind/preact";
import { style as react } from "stillwind/react";
import { style as solid } from "stillwind/solid";
import { style as svelte } from "stillwind/svelte";
import { style as vue } from "stillwind/vue";

type Assert<T extends true> = T;

type Same<A, B> = [A, B] extends [B, A] ? true : false;

type ClassProps<T> = "class" extends keyof T ? ("className" extends keyof T ? false : true) : false;

export type FrameworkSpreadContracts = [
  Assert<ClassProps<ReturnType<typeof astro>>>,
  Assert<ClassProps<ReturnType<typeof preact>>>,
  Assert<ClassProps<ReturnType<typeof solid>>>,
  Assert<ClassProps<ReturnType<typeof svelte>>>,
  Assert<ClassProps<ReturnType<typeof vue>>>,
  Assert<"className" extends keyof ReturnType<typeof react> ? true : false>,
  Assert<typeof css extends string ? true : false>,
];

// Wrappers compose recursively and preserve the public argument contract.
function variantWrapperTypes(active: boolean) {
  const anchor = react.ref("anchor");
  const transition = react.viewTransition({ old: "opacity-0" });
  react({ "hover:": [anchor.positionAnchor(), active && transition.for("hero")] });
  react([
    anchor,
    { "aria-busy": active, "--size": 4, class: "external" },
    active && transition.for("hero"),
  ]);
  react({ "hover:": ["text-red", { "[&:where(.mod)]:": "text-blue" }, false, null, undefined] });
  vue({ "aria-busy:": active && "opacity-60" });
  // @ts-expect-error Raw CSS selectors are not variant wrappers.
  react({ "&:hover": "text-red" });
  // @ts-expect-error Attributes cannot be nested inside a variant wrapper.
  react({ "hover:": { "aria-busy": true } });
}
void variantWrapperTypes;

function refVariantTypes(active: boolean) {
  const trigger = react.ref("trigger");
  react({ [trigger()]: "text-accent" });
  react(`${trigger()}:text-accent`);
  react({ [trigger`hover:active:`]: "text-accent" });
  react({ [trigger("hover:active")]: ["text-accent", { "active:": "opacity-50" }] });
  react({ "md:": { [trigger("hover")]: { "active:": "text-accent" } } });
  react(`${trigger`hover:`}:active:text-accent`);
  react(active && `${trigger("hover")}:text-accent`);
  // @ts-expect-error Ref variant arguments are static strings.
  trigger(123);
  // @ts-expect-error Ref tags do not accept substitutions.
  void trigger`hover:${"active"}`;
}
void refVariantTypes;

function declarationLabelTypes(active: boolean) {
  react.ref();
  react.ref("trigger");
  react.ref("");
  react.keyframes({ label: "fade", from: "opacity-0", to: "opacity-100" });
  react.viewTransition({ label: "card", old: "opacity-0", new: "opacity-100" });
  // @ts-expect-error A ref label is a literal string.
  react.ref(123);
  // @ts-expect-error A keyframes label is a literal string.
  react.keyframes({ label: active, from: "opacity-0" });
  // @ts-expect-error A view-transition label is a literal string.
  react.viewTransition({ label: active, old: "opacity-0" });
}
void declarationLabelTypes;

function customPropertyTypes(image: URL, progress: number) {
  react("w-[calc(var(--progress)*100%)]", {
    "--progress": progress,
    "--image": image,
    "--enabled": false,
  });
  svelte({ "--text": "value" });
  // @ts-expect-error Custom property values cannot be arbitrary objects.
  react({ "--invalid": {} });
  // @ts-expect-error Inline properties cannot be nested in CSS variants.
  react({ "hover:": { "--progress": 1 } });
}
void customPropertyTypes;

function declarationIdentityTypes(
  identity: import("stillwind/react").StillwindDeclarationIdentity<"trigger">,
) {
  const unlabeled = react.ref(identity);
  const labeled = react.ref("trigger", identity);
  type Contracts = [
    Assert<Same<typeof unlabeled, StyleRef<"trigger">>>,
    Assert<Same<typeof labeled, StyleRef<"trigger">>>,
  ];
  react(labeled, labeled.asAnchor());
  svelte(labeled, labeled.positionAnchor());
  vue({ [labeled("hover")]: "text-accent" });
  return null as unknown as Contracts;
}
void declarationIdentityTypes;

function scopedRefTypes(
  r: StyleRef<"r">,
  inner: StyleRef<"inner">,
  outer: StyleRef<"outer">,
  root: StyleRef<"root">,
  top: StyleRef<"top">,
) {
  react({ [r().in(inner)]: "block" });
  react({ [r("hover").in(inner)]: "block" });
  react({ [r`hover:`.in(inner)]: "block" });
  react({ [r.in(inner)]: "block" });
  react({ [r("hover").in(inner("focus"))]: "block" });
  react({
    [r()
      .in(inner())
      .in(outer`focus:`)
      .in(root)
      .in(top())]: "block",
  });
  react(
    `${r`hover`
      .in(inner`focus`)
      .in(outer)
      .in(root)
      .in(top)}:active:block`,
  );
  react({ "md:": { [r.in(inner).in(outer)]: ["block", { "hover:": "flex" }] } });
  preact({ [r.in(inner)]: "block" });
  solid({ [r().in(inner())]: "block" });
  vue({ [r`hover`.in(inner`focus`)]: "block" });
  svelte(`${r.in(inner).in(outer)}:block`);
  astro({ [r.at`width >= 40rem`]: "block" });

  // @ts-expect-error Scope operands must identify ref elements.
  r.in("inner");
  // @ts-expect-error A scope operand must be unscoped.
  r.in(inner.in(outer));
  // @ts-expect-error A ref selector cannot take a scoped scope operand.
  r().in(inner().in(outer));
  // @ts-expect-error Further outer scopes must also be unscoped.
  r.in(inner).in(outer().in(root));
  // @ts-expect-error Container queries cannot supply scopes.
  r.in(inner.at());
  // @ts-expect-error Container query tags cannot supply selector scopes.
  r().in(inner.at`width >= 40rem`);
  // @ts-expect-error Container query calls cannot supply outer scopes.
  r.in(inner).in(outer.at("width >= 40rem"));
  // @ts-expect-error Container queries apply to raw refs.
  r().at();
  // @ts-expect-error Completed variants cannot call the inherited String.at method.
  r().at(0);
  // @ts-expect-error Container queries apply to raw refs.
  r`hover`.at("width >= 40rem");
  // @ts-expect-error Scoped selectors cannot become container queries.
  r.in(inner).at();
  // @ts-expect-error Container queries cannot be scoped.
  r.at().in(inner);
  // @ts-expect-error Container queries cannot be chained.
  r.at().at();
  // @ts-expect-error Completed container variants cannot call String.at.
  r.at().at(0);
  // @ts-expect-error Container queries cannot be called.
  r.at()();
  // @ts-expect-error Container queries cannot be tagged.
  void r.at()``;
  // @ts-expect-error Selector variants must be supplied before scoping.
  r.in(inner)("hover");
  // @ts-expect-error Selector variants must be supplied before scoping.
  void r().in(inner)`hover`;
  // @ts-expect-error An unscoped selector is already a completed ref call.
  r()();
  // @ts-expect-error .in accepts exactly one scope.
  r.in(inner, outer);
  // @ts-expect-error .in requires a scope.
  r().in();
  // @ts-expect-error Ref tags do not accept substitutions.
  react({ [r`hover:${"focus"}`.in(inner)]: "block" });
  // @ts-expect-error Scope tags do not accept substitutions.
  react({ [r.in(inner`hover:${"focus"}`)]: "block" });
  // @ts-expect-error Container query tags do not accept substitutions.
  react({ [r.at`width >= ${40}rem`]: "block" });
}
void scopedRefTypes;

function finiteRefIdentityTypes(
  flag: boolean,
  a: StyleRef<"a">,
  b: StyleRef<"b">,
  inner: StyleRef<"inner">,
  outer: StyleRef<"outer">,
) {
  const alias = a;
  const selected = flag ? alias : b;
  const selectedScope = flag ? inner : outer;
  react({ [selected.in(selectedScope)]: "block" });
  react({
    [selected("hover")
      .in(selectedScope`focus`)
      .in(alias)]: "flex",
  });
  react(`${selected.at`width >= 40rem`}:block`);

  type RefIdOf<T> = T extends StyleRef<infer Id> ? Id : never;
  type VariantIdOf<T> = T extends RefVariant<infer Id> ? Id : never;
  type ContainerIdOf<T> = T extends ContainerVariant<infer Id> ? Id : never;
  type ScopeFactsOf<T> =
    T extends ScopedRefVariant<infer Id, infer Scopes> ? readonly [Id, Scopes] : never;
  type Contracts = [
    Assert<Same<RefIdOf<typeof alias>, "a">>,
    Assert<Same<RefIdOf<typeof selected>, "a" | "b">>,
    Assert<Same<VariantIdOf<ReturnType<typeof selected>>, "a" | "b">>,
    Assert<Same<ContainerIdOf<ReturnType<typeof selected.at>>, "a" | "b">>,
    Assert<
      Same<ScopeFactsOf<ReturnType<typeof selected.in>>, readonly ["a" | "b", readonly [string]]>
    >,
  ];

  // These inferred types check compiler identity metadata. Macro uses in application
  // code must be written directly inside style().
  const unscoped = selected("hover");
  const scoped = unscoped.in(selectedScope);
  const nested = scoped.in(alias("focus")).in(outer`active`);
  type InferredContracts = [
    Assert<Same<VariantIdOf<typeof unscoped>, "a" | "b">>,
    Assert<Same<ScopeFactsOf<typeof scoped>[0], "a" | "b">>,
    Assert<Same<ScopeFactsOf<typeof scoped>[1], readonly ["inner" | "outer"]>>,
    Assert<
      Same<
        ScopeFactsOf<typeof nested>,
        | readonly ["a", readonly ["inner" | "outer", "a", "outer"]]
        | readonly ["b", readonly ["inner" | "outer", "a", "outer"]]
      >
    >,
  ];
  return null as unknown as [Contracts, InferredContracts];
}
void finiteRefIdentityTypes;

function genericRefIdentityTypes<RefId extends string, ScopeId extends string>(
  ref: StyleRef<RefId>,
  scope: StyleRef<ScopeId>,
) {
  react({ [ref.in(scope)]: "block" });
  react(`${ref`hover`.in(scope`focus`)}:block`);

  const selector: RefVariant<RefId> = ref("hover");
  const scoped: ScopedRefVariant<RefId, readonly [ScopeId]> = selector.in(scope);
  const repeated: ScopedRefVariant<RefId, readonly [ScopeId, ScopeId]> = scoped.in(scope());
  const container: ContainerVariant<RefId> = ref.at("width >= 40rem");
  void [repeated, container];
}
void genericRefIdentityTypes;

function explicitRefUses(flag: boolean) {
  const a = react.ref();
  const b = react.ref();
  const selected = flag ? a : b;
  react(selected.positionAnchor());
  /* eslint-disable @typescript-eslint/unbound-method */
  // @ts-expect-error positionAnchor must be called
  react(a.positionAnchor);
  /* eslint-enable @typescript-eslint/unbound-method */
  // @ts-expect-error positionAnchor takes no arguments
  a.positionAnchor("unused");
  react(selected, selected.asAnchor([a, selected]), selected.asContainer());
  react(react.anchorScope([a, b] as const), react.anchorScope("none"));
  react(a, a.asContainer("scroll-state size"), a.asAnchor("all"));
  react(a, a.asContainer("inline-size scroll-state"));
  react(a, a.asContainer("revert-rule"));
  react({ [a.at()]: "block", [a.at("")]: "flex", [a.at``]: "grid" });
  react(`${selected.at()}:hover:block`);
  // @ts-expect-error mutually exclusive size modes
  a.asContainer("size inline-size");
  // @ts-expect-error normal cannot combine with scroll-state
  a.asContainer("normal scroll-state");
  // @ts-expect-error scope identities must be refs
  react.anchorScope(["a"]);
  const query: string = react.media("sm:motion-reduce", "print");
  void query;
  // @ts-expect-error media requires at least one variant list
  react.media();
}
void explicitRefUses;
