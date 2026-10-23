import { style as astro } from "stillwind/astro";
import type { StyleArgument, StyleResult, StyleVariantValue } from "stillwind/macro";
import { style as preact } from "stillwind/preact";
import { style as react } from "stillwind/react";
import { style as solid } from "stillwind/solid";
import { style as svelte } from "stillwind/svelte";
import { style as vanilla } from "stillwind/vanilla";
import { style as vue } from "stillwind/vue";

type Assert<T extends true> = T;
type Same<A, B> = [keyof A, keyof B] extends [keyof B, keyof A]
  ? [A, B] extends [B, A]
    ? true
    : false
  : false;
type Props<T> = Omit<T, "data-__stillwindStyleIdentity">;

function classOutputs(active: boolean) {
  const empty = react();
  const external = react({
    class: {
      external: active,
    },
  });
  const branches = react(active ? "block" : "hidden", [
    "flex",
    {
      "aria-busy:": "opacity-50",
    },
  ]);
  type Contracts = [
    Assert<Same<Props<typeof empty>, { className?: string }>>,
    Assert<Same<Props<typeof external>, { className?: string }>>,
    Assert<Same<Props<typeof branches>, { className?: string }>>,
  ];
  return null as unknown as Contracts;
}
void classOutputs;

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
  const base = react("block");
  const styled = react("flex");
  const composed = react(base, "grid");
  const conditional = react(active && base);
  const selected = react(active ? base : styled);
  const chain = react(composed, styled);
  const identity = react(base, { __stillwindDeclarationIdentity: "derived" });
  const inline = react({ "--size": 4 });
  // @ts-expect-error Results with inline style cannot be composition bases.
  react(inline);
  type Contracts = [
    Assert<Same<Props<typeof composed>, { className?: string }>>,
    Assert<Same<Props<typeof conditional>, { className?: string }>>,
    Assert<Same<Props<typeof selected>, { className?: string }>>,
    Assert<Same<Props<typeof chain>, { className?: string }>>,
    Assert<Same<(typeof identity)["data-__stillwindStyleIdentity"], "derived">>,
    Assert<Same<Props<typeof identity>, Props<typeof base>>>,
    Assert<typeof identity extends StyleResult<"derived"> ? true : false>,
  ];
  return null as unknown as Contracts;
}
void composedOutputs;

function composedFrameworkOutputs(active: boolean) {
  const reactBase = react("block");
  const svelteBase = svelte("block");
  const vanillaBase = vanilla("block");
  const reactResult = react(active ? svelteBase : vanillaBase);
  const svelteResult = svelte([active && reactBase, vanillaBase], { "--tone": "red" });
  const vanillaResult = vanilla(reactBase, svelteBase, { "--tone": "red" });
  const stringInline = svelte({ "--tone": "red" });
  const objectInline = vanilla({ "--tone": "red" });
  // @ts-expect-error A string style cannot be carried over through composition.
  react(stringInline);
  // @ts-expect-error An object style cannot be carried over through composition.
  svelte(objectInline);
  type Contracts = [
    Assert<Same<Props<typeof reactResult>, { className?: string }>>,
    Assert<Same<Props<typeof svelteResult>, { class?: string; style?: string }>>,
    Assert<Same<Props<typeof vanillaResult>, { class?: string; style?: Record<string, string> }>>,
  ];
  return null as unknown as Contracts;
}
void composedFrameworkOutputs;

function frameworkOutputs() {
  const astroPlain = astro("block");
  const preactPlain = preact("block");
  const solidPlain = solid("block");
  const sveltePlain = svelte("block");
  const vuePlain = vue("block");
  const vanillaPlain = vanilla("block");
  const astroCustom = astro({ "--size": 4 });
  const preactCustom = preact({ "--size": 4 });
  const solidCustom = solid({ "--size": 4 });
  const svelteCustom = svelte({ "--size": 4 });
  const vueCustom = vue({ "--size": 4 });
  const vanillaCustom = vanilla({ "--size": 4 });
  type Contracts = [
    Assert<Same<Props<typeof astroPlain>, { class?: string }>>,
    Assert<Same<Props<typeof preactPlain>, { class?: string }>>,
    Assert<Same<Props<typeof solidPlain>, { class?: string }>>,
    Assert<Same<Props<typeof sveltePlain>, { class?: string }>>,
    Assert<Same<Props<typeof vuePlain>, { class?: string }>>,
    Assert<Same<Props<typeof vanillaPlain>, { class?: string }>>,
    Assert<
      Same<Props<typeof astroCustom | typeof svelteCustom>, { class?: string; style?: string }>
    >,
    Assert<
      Same<
        Props<typeof preactCustom | typeof solidCustom | typeof vueCustom | typeof vanillaCustom>,
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
  inputs: readonly StyleResult<string, { className?: string }>[],
  rules: StyleVariantValue,
  tuple: readonly [StyleResult<string, { className?: string }>?],
) {
  const array = react(...inputs);
  const base = react("block");
  const arrayAfterBase = react(base, ...inputs);
  const wrapper = react({ "hover:": rules });
  const optionalTuple = react(...tuple);
  type Contracts = [
    Assert<Same<Props<typeof array>, { className?: string }>>,
    Assert<Same<Props<typeof arrayAfterBase>, { className?: string }>>,
    Assert<Same<Props<typeof wrapper>, { className?: string; style?: Record<string, string> }>>,
    Assert<Same<Props<typeof optionalTuple>, { className?: string }>>,
  ];
  return null as unknown as Contracts;
}
void variableInputs;

function invalidInputs() {
  const ref = react.ref();
  const base = react("block");
  // @ts-expect-error Unknown object entries are rejected.
  react({ title: "card" });
  // @ts-expect-error Authored inline style is not a macro input.
  react({ style: { color: "red" } });
  // @ts-expect-error Inline properties cannot occur in a variant wrapper.
  react({ "hover:": [{ "--size": 4 }] });
  // @ts-expect-error Empty rule groups accept the same inputs as variant wrappers.
  react({ "": { "--size": 4 } });
  // @ts-expect-error Ref attachments require positions outside rule groups.
  react({ ":": ref });
  // @ts-expect-error Composed style props require positions outside rule groups.
  react({ "": base });
}
void invalidInputs;
