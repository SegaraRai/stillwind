import { style as astro } from "stillwind/astro";
import css from "stillwind/module-css";
import { style as preact } from "stillwind/preact";
import { style as react } from "stillwind/react";
import { style as solid } from "stillwind/solid";
import { style as svelte } from "stillwind/svelte";
import { style as vue } from "stillwind/vue";

type Assert<T extends true> = T;
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
  react({ [trigger`hover:active:`]: "text-accent" });
  react({ [trigger("hover:active")]: ["text-accent", { "active:": "opacity-50" }] });
  react({ "md:": { [trigger("hover")]: { "active:": "text-accent" } } });
  react(`${trigger`hover:`}:active:text-accent`);
  react(active && `${trigger("hover")}:text-accent`);
  // @ts-expect-error Ref variant arguments are static strings.
  trigger(123);
  // @ts-expect-error Ref tags do not accept substitutions.
  trigger`hover:${"active"}`;
}
void refVariantTypes;
