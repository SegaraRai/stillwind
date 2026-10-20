/** Framework macros and the placeholders that the compiler replaces. */

import type { StyleCssInput, StyleCssOptions } from "./css.ts";
import type {
  CvaClassProps,
  CvaConfig,
  CvaInput,
  CvaResult,
  CvaSelection,
  CvaVariants,
} from "./cva.ts";
import type { CheckedStyleInputs, StyleArgument } from "./input.ts";
import type { StyleKeyframes, StyleKeyframesDefinition } from "./keyframes.ts";
import type { StyleOutput, StyleResult } from "./output.ts";
import type { StyleRefProjection } from "./projection.ts";
import type { AnchorScope, StyleRef } from "./refs.ts";
import type { StyleViewTransition, StyleViewTransitionDefinition } from "./view-transition.ts";

/**
 * Internal use only. The compiler inserts this identity argument in source used by the
 * TypeScript checker. Application code must not construct or pass it.
 *
 * @internal
 */
export type StillwindDeclarationIdentity<Id extends string> = {
  readonly __stillwindDeclarationIdentity: Id;
};

/**
 * Compile-time UI declarations with pure value expressions.
 *
 * Only completed style calls and declaration macros produce reusable values.
 * Projections require their supported style input positions. Ref variants require
 * computed variant keys or utility template substitutions.
 *
 * Each argument expression must return an equivalent value for the same inputs and must
 * not mutate shared data or perform observable effects, including through callees,
 * getters, or value conversion. Calls need no PURE annotation. Functions may mutate
 * fresh local data used only to compute their result.
 *
 * Evaluation order and count are not API guarantees: unused expressions may be omitted
 * and class/value conversions may be combined. Do not rely on exceptions from unused
 * expressions. Read time/randomness and perform I/O, state updates, Hooks, await/yield,
 * imports, JSX, and class initialization outside the macro. Pass their resulting
 * values. Reactive reads stay in the current computation scope and must not be used
 * only to establish a subscription.
 *
 * Names, utility definitions, animation options, and selector/query arguments are
 * static declarations. Dynamic identity selectors and view-transition keys obey the
 * same value-expression contract. Declaration-identity overloads are internal checker
 * metadata.
 */
export type StyleMacro<Props = object> = {
  /**
   * Internal use only. The checker supplies the final identity argument to track the
   * completed props value. Application code must use the overload without it.
   *
   * @internal
   */
  <Inputs extends readonly StyleArgument[], Id extends string>(
    ...inputs: [...Inputs, StillwindDeclarationIdentity<Id>] &
      [...CheckedStyleInputs<Inputs>, StillwindDeclarationIdentity<Id>]
  ): StyleResult<Id, StyleOutput<Props, Inputs>>;

  /**
   * Compiles static utility branches and combines them with inline custom
   * properties, external classes, and completed style props.
   *
   * Value expressions must obey the purity contract of {@link StyleMacro}. Projection
   * and animation-use calls must appear directly at supported input positions. Ref
   * variants require computed keys or utility template substitutions.
   *
   * @param inputs - Rule and class inputs in composition order. Custom properties
   * replace earlier values, and classes are concatenated. Utility construction must be
   * static, and invalid utilities or unsupported syntax cause compilation errors.
   * @returns Framework props that may be stored, exported, passed, returned, or spread
   * onto an element. Store this result before passing it to another `style()` call.
   * Direct nested calls are rejected. The class prop is optional. The optional style
   * prop is present in the type when inputs include custom properties, participant
   * naming, or composed props that may contain inline style. CSS is emitted at build
   * time.
   */
  <Inputs extends readonly StyleArgument[]>(
    ...inputs: Inputs & CheckedStyleInputs<Inputs>
  ): StyleResult<string, StyleOutput<Props, Inputs>>;

  /**
   * Declares an unlabeled DOM identity in a binding initializer.
   * Passing the declaration to `style()` attaches its marker to that element.
   *
   * @returns A reusable ref declaration that may be stored, exported, passed, returned,
   * or conditionally selected. Its identity belongs to the declaration site. Repeated
   * evaluation does not allocate new CSS identities.
   */
  ref(): StyleRef;

  /**
   * Declares a labeled DOM identity in a binding initializer.
   * Passing the declaration to `style()` attaches its marker to that element.
   *
   * @param label - A string literal used in generated names. Empty text omits the
   * label. Labels do not determine declaration identity.
   * @returns A reusable ref declaration that may be stored, exported, passed, returned,
   * or conditionally selected. Repeated evaluation keeps its identity.
   */
  ref(label: string): StyleRef;

  /**
   * Internal use only. The checker supplies identity for an unlabeled ref declaration.
   * Application code must use `ref()` without this argument.
   *
   * @internal
   */
  ref<const Id extends string>(identity: StillwindDeclarationIdentity<Id>): StyleRef<Id>;

  /**
   * Internal use only. The checker supplies identity for a labeled ref declaration.
   * Application code must use `ref(label)` without the identity argument.
   *
   * @internal
   */
  ref<const Id extends string>(
    label: string,
    identity: StillwindDeclarationIdentity<Id>,
  ): StyleRef<Id>;

  /**
   * Declares a local animation in a binding initializer and assigns its CSS name.
   *
   * Apply it by calling the returned declaration with literal options directly inside
   * `style()`. The animation use cannot be stored for later consumption.
   *
   * @param definition - A direct static object with frame utility strings, optional
   * media/supports replacement bodies, and an optional literal label. Invalid frame
   * syntax or utilities cause compilation errors.
   * @returns A reusable keyframes declaration that may be stored, exported, passed,
   * returned, or conditionally selected. Its identity belongs to the declaration site.
   */
  keyframes(definition: StyleKeyframesDefinition): StyleKeyframes;

  /**
   * Internal use only. The checker supplies identity for a keyframes declaration.
   * Application code must use `keyframes(definition)` without the identity argument.
   *
   * @internal
   */
  keyframes<const Id extends string>(
    definition: StyleKeyframesDefinition,
    identity: StillwindDeclarationIdentity<Id>,
  ): StyleKeyframes<Id>;

  /**
   * Declares view transition phases in a binding initializer and assigns their class.
   *
   * Pass the declaration to `style()` to mark participants, or call `.for(key)`
   * directly inside `style()` to also name them. The naming use cannot be stored.
   *
   * @param definition - A direct static object with at least one literal phase
   * string and an optional literal label. Invalid phases, utilities, or condition
   * variants cause compilation errors.
   * @returns A reusable transition declaration that may be stored, exported, passed,
   * returned, or conditionally selected. Its identity belongs to the declaration site.
   */
  viewTransition(definition: StyleViewTransitionDefinition): StyleViewTransition;

  /**
   * Internal use only. The checker supplies identity for a view transition declaration.
   * Application code must use `viewTransition(definition)` without the identity argument.
   *
   * @internal
   */
  viewTransition<const Id extends string>(
    definition: StyleViewTransitionDefinition,
    identity: StillwindDeclarationIdentity<Id>,
  ): StyleViewTransition<Id>;

  /**
   * Adds anchor-scope without requiring a ref marker on the element.
   *
   * Use this call directly in a style input. The argument follows the same literal
   * keyword or array rules as `ref.asAnchor`. An empty array emits no declaration.
   * The returned projection cannot be stored, exported, passed, or returned for later
   * consumption.
   *
   * @param scope - A literal `"all"`, `"none"`, or array of refs. All possible
   * identities of each entry are included. Empty arrays and duplicate identities
   * produce lint findings.
   * @returns Rule input that supplies anchor-scope when consumed by `style()`.
   */
  anchorScope(scope: AnchorScope): StyleRefProjection;

  /**
   * Returns the media query list of variants, such as for `window.matchMedia()`.
   *
   * The compiler replaces the call with a string, so the result can be stored, exported,
   * and passed. The string has no `@media` or braces. Write the call outside other macro
   * calls.
   *
   * @param variants - String literals of variants whose definitions contain only media
   * conditions, with an optional final `:`. The variants of one argument must all hold,
   * and the arguments are alternatives: `media("sm:motion-reduce", "print")` matches a
   * wide screen with reduced motion, or print. An argument that does not resolve or that
   * contains another kind of condition, such as `hover`, matches nothing and produces a
   * lint finding.
   * @returns A media query list. It is `"not all"` when no argument can match.
   */
  media(...variants: [string, ...string[]]): string;

  /**
   * Returns a stylesheet built from utilities, such as for a shadow root.
   *
   * The compiler replaces the call with a string, so the result can be stored,
   * exported, and passed, for example to `CSSStyleSheet.replaceSync()`. The call adds
   * nothing to the document stylesheet. The string contains the rules and the theme
   * values, keyframes, and custom-property support that they need, placed by the
   * selected `@stillwind-css-template`. It never contains preflight.
   *
   * A selector-replacing variant, such as `host:`, `[:host]:`, or `[button]:`, selects
   * the element that a utility styles. Utilities without one need the `selector`
   * option, or are omitted with a lint finding. Write the call outside other macro
   * calls.
   *
   * @param inputs - Static utility strings, arrays, and variant groups. A final
   * object whose keys are only `template` and `selector` is the options.
   * @returns Stylesheet text.
   */
  css(...inputs: StyleCssInput[]): string;

  /**
   * Returns a stylesheet built from utilities, with the options in the last argument.
   *
   * @param inputs - Static utility inputs followed by the options object.
   * @returns Stylesheet text.
   */
  css(...inputs: [...StyleCssInput[], StyleCssOptions]): string;

  /**
   * Compiles a CVA definition and returns a function that selects element props.
   * Write the configuration and utility inputs directly in the call. Custom properties
   * remain inline and value expressions obey this macro's purity contract.
   *
   * @param base - Optional styles included in every selection.
   * @param config - Static variants, defaults, and compound conditions.
   * @returns A reusable factory returning fresh framework class and style props.
   * Its results cannot be passed to style() for composition.
   */
  cva<Variants extends CvaVariants = Record<never, never>>(
    base?: CvaInput,
    config?: CvaConfig<Variants>,
  ): (props?: CvaSelection<Variants> & CvaClassProps) => CvaResult<Props>;
};

/**
 * Internal use only. Builds the placeholder exported by each framework entrypoint.
 * The compiler replaces supported macro calls before the application runs.
 *
 * @returns A fresh callable object with declaration and scope methods. Calling the
 * object or any of these methods throws an Error about missing integration. Creating
 * the placeholder itself does not execute any macro or generate CSS.
 *
 * @internal
 */
export function createStyleMacro<Props>(): StyleMacro<Props> {
  return Object.assign(
    (): never => {
      throw new Error(
        "Stillwind style() is a compile-time macro. Configure the Stillwind integration for your build tool.",
      );
    },
    {
      anchorScope(): never {
        throw new Error("Stillwind style.anchorScope() is a compile-time macro.");
      },
      ref(): never {
        throw new Error("Stillwind style.ref() is a compile-time macro.");
      },
      keyframes(): never {
        throw new Error("Stillwind style.keyframes() is a compile-time macro.");
      },
      viewTransition(): never {
        throw new Error("Stillwind style.viewTransition() is a compile-time macro.");
      },
      cva(): never {
        throw new Error("Stillwind style.cva() is a compile-time macro.");
      },
      media(): never {
        throw new Error("Stillwind style.media() is a compile-time macro.");
      },
      css(): never {
        throw new Error("Stillwind style.css() is a compile-time macro.");
      },
    },
  );
}
