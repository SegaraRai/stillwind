import { style as astro } from "stillwind/astro";
import type { StyleArgument, StyleResult, StyleVariantValue } from "stillwind/macro";
import { style as preact } from "stillwind/preact";
import { style as react } from "stillwind/react";
import { style as solid } from "stillwind/solid";
import { style as svelte } from "stillwind/svelte";
import { style as vue } from "stillwind/vue";

type Assert<T extends true> = T;
type Same<A, B> = [keyof A, keyof B] extends [keyof B, keyof A]
  ? [A, B] extends [B, A]
    ? true
    : false
  : false;
type Props<T> = Omit<T, "data-__stillwindStyleIdentity">;
type Required<T, Key extends keyof T> = T extends Record<Key, unknown> ? true : false;

function attributeOutputs(active: boolean, count: number, value: { id: number }) {
  const plain = react("block");
  const empty = react();
  const attributes = react({ "data-count": count, "aria-busy": active, "data-value": value });
  const literals = react({ "data-mode": "card", "aria-hidden": false, "data-empty": undefined });
  const conditional = react(active && { "data-count": count });
  const selected = react(active ? { "data-mode": "card" } : { "aria-hidden": true });
  const alternatives = react(active ? { "data-value": 1 } : { "data-value": "two" });
  const nested = react(["block", [{ "data-count": count }, active && { "aria-busy": active }]]);
  const overwritten = react({ "data-value": 1 }, { "data-value": "two" });
  const maybeOverwritten = react({ "data-value": 1 }, active && { "data-value": "two" });
  const undefinedOverwrite = react({ "data-value": 1 }, { "data-value": undefined });
  const conditionalUndefined = react({ "data-value": 1 }, active && { "data-value": undefined });
  const classInput = react({ class: { "external-class": active } });
  const attributeVariants = react({ "aria-busy:": "block", "data-active:": "flex" });
  const objectRules = react({
    "": ["block", active && "flex"],
    ":": { "hover:": "hidden" },
    "data-count": count,
  });
  type Contracts = [
    Assert<Same<Props<typeof plain>, { className?: string }>>,
    Assert<Same<Props<typeof empty>, { className?: string }>>,
    Assert<
      Same<
        Props<typeof attributes>,
        {
          className?: string;
          "data-count": number;
          "aria-busy": boolean;
          "data-value": { id: number };
        }
      >
    >,
    Assert<
      Same<
        Props<typeof literals>,
        { className?: string; "data-mode": "card"; "aria-hidden": false; "data-empty": undefined }
      >
    >,
    Assert<Same<Props<typeof conditional>, { className?: string; "data-count"?: number }>>,
    Assert<
      Same<
        Props<typeof selected>,
        { className?: string; "data-mode"?: "card"; "aria-hidden"?: true }
      >
    >,
    Assert<Same<(typeof alternatives)["data-value"], 1 | "two">>,
    Assert<Required<typeof alternatives, "data-value">>,
    Assert<
      Same<Props<typeof nested>, { className?: string; "data-count": number; "aria-busy"?: true }>
    >,
    Assert<Same<(typeof overwritten)["data-value"], "two">>,
    Assert<Same<(typeof maybeOverwritten)["data-value"], 1 | "two">>,
    Assert<Required<typeof maybeOverwritten, "data-value">>,
    Assert<Same<(typeof undefinedOverwrite)["data-value"], undefined>>,
    Assert<Required<typeof undefinedOverwrite, "data-value">>,
    Assert<Same<(typeof conditionalUndefined)["data-value"], 1 | undefined>>,
    Assert<Required<typeof conditionalUndefined, "data-value">>,
    Assert<Same<Props<typeof classInput>, { className?: string }>>,
    Assert<Same<Props<typeof attributeVariants>, { className?: string }>>,
    Assert<Same<Props<typeof objectRules>, { className?: string; "data-count": number }>>,
  ];
  return null as unknown as Contracts;
}
void attributeOutputs;

function inlineStyleOutputs(active: boolean, key: string) {
  const transition = react.viewTransition({ old: "opacity-0" });
  const ref = react.ref();
  const frames = react.keyframes({ from: "opacity-0" });
  const rules = react(ref, ref.asAnchor(), frames("duration-200"), transition);
  const custom = react({ "--size": 4 });
  const conditional = react(active && { "--size": 4 });
  const named = react(transition.for(key));
  const nested = react({ "hover:": [{ [ref()]: active && transition.for(key) }] });
  const computed = react({ [ref()]: transition.for(key) });
  const computedRules = react({ [ref()]: "block" });
  const external = react({ class: { "--class": active, style: active } });
  const groupedRules = react({ "": [ref.asAnchor(), frames("duration-200")], ":": false });
  const groupedNamed = react({ "": transition.for(key) });
  const variantNamed = react({ "hover:": transition.for(key) });
  const variantDeclaration = react({ "hover:": transition });
  type Contracts = [
    Assert<Same<Props<typeof rules>, { className?: string }>>,
    Assert<Same<Props<typeof custom>, { className?: string; style?: Record<string, string> }>>,
    Assert<Same<Props<typeof conditional>, { className?: string; style?: Record<string, string> }>>,
    Assert<Same<Props<typeof named>, { className?: string; style?: Record<string, string> }>>,
    Assert<Same<Props<typeof nested>, { className?: string; style?: Record<string, string> }>>,
    Assert<Same<Props<typeof computed>, { className?: string; style?: Record<string, string> }>>,
    Assert<Same<Props<typeof computedRules>, { className?: string }>>,
    Assert<Same<Props<typeof external>, { className?: string }>>,
    Assert<Same<Props<typeof groupedRules>, { className?: string }>>,
    Assert<
      Same<Props<typeof groupedNamed>, { className?: string; style?: Record<string, string> }>
    >,
    Assert<
      Same<Props<typeof variantNamed>, { className?: string; style?: Record<string, string> }>
    >,
    Assert<Same<Props<typeof variantDeclaration>, { className?: string }>>,
  ];
  return null as unknown as Contracts;
}
void inlineStyleOutputs;

function composedOutputs(active: boolean) {
  const base = react({ "data-mode": "card", "aria-hidden": false });
  const styled = react({ "--size": 4, "data-size": 4 });
  const composed = react(base, { "data-mode": "panel" });
  const conditional = react(active && base);
  const selected = react(active ? base : styled);
  const chain = react(composed, styled);
  const identity = react(base, { __stillwindDeclarationIdentity: "derived" });
  type Contracts = [
    Assert<
      Same<
        Props<typeof composed>,
        { className?: string; "data-mode": "panel"; "aria-hidden": false }
      >
    >,
    Assert<
      Same<
        Props<typeof conditional>,
        { className?: string; "data-mode"?: "card"; "aria-hidden"?: false }
      >
    >,
    Assert<
      Same<
        Props<typeof selected>,
        {
          className?: string;
          "data-mode"?: "card";
          "aria-hidden"?: false;
          "data-size"?: 4;
          style?: Record<string, string>;
        }
      >
    >,
    Assert<
      Same<
        Props<typeof chain>,
        {
          className?: string;
          "data-mode": "panel";
          "aria-hidden": false;
          "data-size": 4;
          style?: Record<string, string>;
        }
      >
    >,
    Assert<Same<(typeof identity)["data-__stillwindStyleIdentity"], "derived">>,
    Assert<Same<Props<typeof identity>, Props<typeof base>>>,
    Assert<typeof identity extends StyleResult<"derived"> ? true : false>,
  ];
  return null as unknown as Contracts;
}
void composedOutputs;

function frameworkOutputs() {
  const astroPlain = astro("block");
  const preactPlain = preact("block");
  const solidPlain = solid("block");
  const sveltePlain = svelte("block");
  const vuePlain = vue("block");
  const astroCustom = astro({ "--size": 4 });
  const preactCustom = preact({ "--size": 4 });
  const solidCustom = solid({ "--size": 4 });
  const svelteCustom = svelte({ "--size": 4 });
  const vueCustom = vue({ "--size": 4 });
  type Contracts = [
    Assert<Same<Props<typeof astroPlain>, { class?: string }>>,
    Assert<Same<Props<typeof preactPlain>, { class?: string }>>,
    Assert<Same<Props<typeof solidPlain>, { class?: string }>>,
    Assert<Same<Props<typeof sveltePlain>, { class?: string }>>,
    Assert<Same<Props<typeof vuePlain>, { class?: string }>>,
    Assert<
      Same<Props<typeof astroCustom | typeof svelteCustom>, { class?: string; style?: string }>
    >,
    Assert<
      Same<
        Props<typeof preactCustom | typeof solidCustom | typeof vueCustom>,
        { class?: string; style?: Record<string, string> }
      >
    >,
  ];
  return null as unknown as Contracts;
}
void frameworkOutputs;

function broadInputs(inputs: readonly StyleArgument[]) {
  const result = react(...inputs);
  type Contracts = [Assert<Same<Props<typeof result>, import("stillwind/react").StyleProps>>];
  return null as unknown as Contracts;
}
void broadInputs;

function variableInputs(
  inputs: readonly StyleResult<string, { "data-value": number }>[],
  attributes: { "data-count"?: number; "aria-busy": boolean },
  rules: StyleVariantValue,
  tuple: readonly [StyleResult<string, { "data-value": number }>?],
) {
  const array = react(...inputs);
  const arrayAfterBase = react({ "data-value": "base" }, ...inputs);
  const optional = react(attributes);
  const wrapper = react({ "hover:": rules });
  const optionalTuple = react(...tuple);
  type Contracts = [
    Assert<Same<Props<typeof array>, { className?: string; "data-value"?: number }>>,
    Assert<
      Same<Props<typeof arrayAfterBase>, { className?: string; "data-value": "base" | number }>
    >,
    Assert<
      Same<
        Props<typeof optional>,
        { className?: string; "data-count"?: number; "aria-busy": boolean }
      >
    >,
    Assert<Same<Props<typeof wrapper>, { className?: string; style?: Record<string, string> }>>,
    Assert<Same<Props<typeof optionalTuple>, { className?: string; "data-value"?: number }>>,
  ];
  return null as unknown as Contracts;
}
void variableInputs;

function invalidInputs() {
  const ref = react.ref();
  const base = react("block");
  // @ts-expect-error Only data and ARIA attributes are accepted as ordinary props.
  react({ title: "card" });
  // @ts-expect-error Authored inline style is not a macro input.
  react({ style: { color: "red" } });
  // @ts-expect-error Inline properties cannot occur in a variant wrapper.
  react({ "hover:": [{ "--size": 4 }] });
  // @ts-expect-error Attributes cannot occur in a nested variant wrapper.
  react({ "hover:": { "focus:": { "data-state": "active" } } });
  // @ts-expect-error Empty rule groups accept the same inputs as variant wrappers.
  react({ "": { "--size": 4 } });
  // @ts-expect-error Ref attachments require positions outside rule groups.
  react({ ":": ref });
  // @ts-expect-error Composed style props require positions outside rule groups.
  react({ "": base });
}
void invalidInputs;
