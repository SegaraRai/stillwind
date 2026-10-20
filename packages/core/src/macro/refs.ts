/** Ref declarations, scoped selectors, and named container queries used by style(). */

import type { refIdentity, refProjection, StyleRefProjection } from "./projection.ts";

declare const refVariantKind: unique symbol;

/**
 * An explicit scope written as a literal keyword or a literal array of refs.
 *
 * For a ref that can resolve to several identities, the scope includes all of them,
 * regardless of which one is selected at runtime. Duplicate identities and empty arrays
 * produce lint findings.
 *
 * Write the keyword or array directly in `asAnchor()` or `style.anchorScope()`.
 * A stored scope value does not satisfy the compiler's literal syntax contract.
 */
export type AnchorScope = "all" | "none" | readonly StyleRef[];

/**
 * CSS-wide keywords accepted as literal `container-type` values by `asContainer`.
 *
 * Write the keyword directly as a string literal in the macro call.
 * A stored string does not satisfy that syntax contract.
 */
export type CssWideKeyword =
  | "initial"
  | "inherit"
  | "unset"
  | "revert"
  | "revert-layer"
  | "revert-rule";

/**
 * Literal `container-type` values accepted by `asContainer`.
 *
 * A size mode may combine with `scroll-state` in either listed order. Write the string
 * literal directly in the call. A stored string is rejected.
 */
export type ContainerType =
  | CssWideKeyword
  | "normal"
  | "size"
  | "inline-size"
  | "scroll-state"
  | "size scroll-state"
  | "scroll-state size"
  | "inline-size scroll-state"
  | "scroll-state inline-size";

/**
 * A DOM identity attached by passing the ref itself to `style()`.
 *
 * `asAnchor` and `asContainer` add CSS that applies only when the same element has this
 * ref's marker. A ref declaration can also supply a scope to `.in()`. The declaration
 * value may be stored, exported, passed, returned, and selected conditionally. Its
 * projection and variant calls must appear directly inside `style()` and cannot be kept
 * as separate values. `RefId` carries compiler identity. Application code should let
 * the checker infer them.
 */
export type StyleRef<RefId extends string = string> = {
  /** @internal */
  readonly [refIdentity]: {
    readonly ref: RefId;
  };

  /**
   * Supplies this ref's anchor-name and optional anchor-scope.
   *
   * If `scope` is an array, this ref is included only when listed in that array.
   * Use the call directly in a style input and attach the ref to the same element.
   * The returned projection cannot be stored or passed for later use.
   *
   * @param scope - A literal `"all"`, `"none"`, or array of refs. Empty arrays and
   * duplicate identities produce lint findings. An empty array or omitted argument
   * emits no scope declaration.
   * @returns Rule input that adds this ref's anchor-name when consumed by `style()`.
   */
  asAnchor(scope?: AnchorScope): StyleRefProjection<RefId>;

  /**
   * Adds this ref's container-name and, if provided, the literal container-type.
   *
   * Omitting `type` emits no container-type, leaving the CSS cascade to determine it.
   * Use the call directly in a style input and attach the ref to the same element.
   * The returned projection cannot be stored or passed for later use.
   *
   * @param type - An optional string literal from {@link ContainerType}.
   * @returns Rule input that adds this ref's container-name and optional container-type.
   */
  asContainer(type?: ContainerType): StyleRefProjection<RefId>;

  /**
   * Refers to this ref's generated anchor-name from the styled element.
   *
   * Supply an anchor provider with `asAnchor()` on an element carrying this ref.
   * Use the call directly as a rule input in `style()`. The returned projection cannot
   * be stored or passed for later use.
   *
   * @returns Rule input that adds position-anchor to the styled element.
   */
  positionAnchor(): StyleRefProjection<RefId>;

  /**
   * Tests this ref's presence within the supplied scope. This is shorthand for calling
   * this ref without arguments and then applying `.in(scope)`.
   *
   * Use the complete chain directly as a computed variant key or a variant segment
   * followed by `:` in a utility template inside `style()`.
   *
   * @param scope - A raw ref declaration or an unscoped ref call or static tag.
   * The scope ref must be a strict ancestor of both this ref and the styled target.
   * Any variants on the scope operand test that scope element.
   * @returns Scoped selector syntax that accepts further `.in()` calls.
   * The result cannot be stored or passed for later use.
   */
  in<const ScopeId extends string>(
    scope: RefScope<ScopeId>,
  ): ScopedRefVariant<RefId, readonly [ScopeId]>;

  /**
   * Selects this ref's named container without a query condition.
   *
   * Supply its container-name with `asContainer()` on an element carrying this ref.
   * Use the call directly as a computed variant key or a complete variant segment
   * followed by `:` in a utility template inside `style()`.
   *
   * @returns Variant syntax that emits `@container <name>` without parentheses.
   * The result cannot be stored or passed as an independent value.
   */
  at(): ContainerVariant<RefId>;

  /**
   * Applies a literal CSS query to this ref's named container.
   *
   * Supply its container-name with `asContainer()` on an element carrying this ref.
   * Use the call directly as a computed variant key or a complete variant segment
   * followed by `:` in a utility template inside `style()`.
   *
   * @param query - A string literal containing the CSS condition without its outer
   * parentheses. Empty text emits only the container name.
   * @returns Variant syntax for the named container query. The result cannot be
   * stored or passed as an independent value.
   */
  at(query: string): ContainerVariant<RefId>;

  /**
   * Applies a static tagged query to this ref's named container.
   *
   * Supply its container-name with `asContainer()` on an element carrying this ref.
   * Use the tag directly as a computed variant key or a complete variant segment
   * followed by `:` in a utility template inside `style()`.
   *
   * @param strings - Text from a tag such as ``ref.at`width >= 40rem` ``.
   * Interpolations are rejected. Empty text emits only the container name.
   * @returns Variant syntax for the named container query. The result cannot be
   * stored or passed as an independent value.
   */
  at(strings: TemplateStringsArray): ContainerVariant<RefId>;

  /**
   * Tests a static variant sequence on the referenced element.
   *
   * Use the tag directly as a computed variant key or a complete variant segment
   * followed by `:` in a utility template inside `style()`. Variants outside the
   * tag apply to the styled target.
   *
   * @param strings - Text from a tag such as ``ref`hover:active` ``. Its final `:`
   * is optional. Interpolations are rejected. Empty text tests ref presence.
   * @returns Ref variant syntax. The result cannot be stored or passed as an
   * independent value.
   */
  (strings: TemplateStringsArray): RefVariant<RefId>;

  /**
   * Tests ref presence without a state condition. Without `.in()`, this ref must be an
   * ancestor of the styled target. `.in()` scopes the selector explicitly.
   *
   * Use the call directly as a computed variant key or a complete variant segment
   * followed by `:` in a utility template inside `style()`.
   *
   * @returns Ref variant syntax. The result cannot be stored or passed as an
   * independent value.
   */
  (): RefVariant<RefId>;

  /**
   * Tests a literal variant sequence on the referenced element.
   *
   * Use the call directly as a computed variant key or a complete variant segment
   * followed by `:` in a utility template inside `style()`. Variants outside the
   * call apply to the styled target.
   *
   * @param variants - A string literal such as `"hover:active"`, with an optional
   * final `:`. Empty text tests ref presence.
   * @returns Ref variant syntax. The result cannot be stored or passed as an
   * independent value.
   */
  (variants: string): RefVariant<RefId>;
};

/**
 * A scope operand for `.in()`: a raw ref declaration or an unscoped ref call or static
 * tag. Scoped selectors and container queries cannot supply a scope.
 *
 * Declaration values may be reused. Calls and tags must appear directly in the
 * `.in()` chain consumed by `style()`.
 * `RefId` preserves the possible declaration identities of the scope operand.
 */
export type RefScope<RefId extends string = string> = StyleRef<RefId> | RefVariant<RefId>;

export type StyleVariantKey = string & {
  readonly [refProjection]: true;
  readonly at: never;
};

/**
 * An unscoped ref selector returned by a ref call or static tag.
 *
 * Without `.in()`, the referenced element must be an ancestor of the styled target.
 * Use the complete call, tag, or `.in()` chain directly as a computed variant key or a
 * variant segment followed by `:` in a utility template inside `style()`.
 * This syntax cannot be stored, exported, passed, or returned for later use.
 * `RefId` preserves the possible identities of the referenced element.
 */
export type RefVariant<RefId extends string = string> = StyleVariantKey & {
  /** @internal */
  readonly [refVariantKind]: "unscoped";
  /** @internal */
  readonly [refIdentity]: {
    readonly ref: RefId;
  };

  /**
   * Scopes this selector to a strict ancestor of the referenced element and the styled
   * target. Any variants on the scope operand test that scope element.
   *
   * Use the complete chain directly as a computed key or utility variant segment inside
   * `style()`.
   *
   * @param scope - A raw ref declaration or an unscoped ref call or static tag.
   * @returns Scoped selector syntax that accepts further `.in()` calls.
   * The result cannot be stored or passed for later use.
   */
  in<const ScopeId extends string>(
    scope: RefScope<ScopeId>,
  ): ScopedRefVariant<RefId, readonly [ScopeId]>;
};

/**
 * A ref selector with one or more explicit ancestor scopes.
 *
 * Each further `.in()` adds an independent condition that its scope contains both the
 * referenced element and the styled target. The method may be chained any number of
 * times. Use the complete chain directly as a computed key or a variant segment
 * followed by `:` in a utility template inside `style()`.
 *
 * This syntax cannot be stored, exported, passed, or returned for later use.
 * `RefId` preserves the referenced identities. `ScopeIds` preserves each scope's
 * possible identities in the order of the `.in()` calls.
 */
export type ScopedRefVariant<
  RefId extends string = string,
  ScopeIds extends readonly string[] = readonly string[],
> = StyleVariantKey & {
  /** @internal */
  readonly [refVariantKind]: "scoped";
  /** @internal */
  readonly [refIdentity]: {
    readonly ref: RefId;
    readonly scopes: ScopeIds;
  };

  /**
   * Adds a scope that must be a strict ancestor of the referenced element and the
   * styled target. Each scope condition applies independently. Any variants on
   * the operand test that scope element.
   *
   * Use the complete chain directly as a computed key or utility variant segment
   * inside `style()`.
   *
   * @param scope - A raw ref declaration or an unscoped ref call or static tag.
   * @returns Scoped selector syntax that preserves the existing scopes and accepts
   * further `.in()` calls. The result cannot be stored or passed for later use.
   */
  in<const ScopeId extends string>(
    scope: RefScope<ScopeId>,
  ): ScopedRefVariant<RefId, readonly [...ScopeIds, ScopeId]>;
};

/**
 * A named container query returned by a raw ref's `.at()` call or static tag.
 * Supply the container name with `.asContainer()` on an element carrying that ref.
 *
 * Use the call or tag directly as a computed key or a variant segment followed by
 * `:` in a utility template inside `style()`. This syntax cannot be stored,
 * exported, passed, or returned for later use. It accepts no further macro methods.
 * `RefId` preserves the possible identities of the container provider.
 */
export type ContainerVariant<RefId extends string = string> = StyleVariantKey & {
  /** @internal */
  readonly [refVariantKind]: "container";
  /** @internal */
  readonly [refIdentity]: {
    readonly ref: RefId;
  };
};
