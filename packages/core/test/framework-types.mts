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
  trigger`hover:${"active"}`;
}
void refVariantTypes;

// Declarations accept an optional literal label beside their own entries, and a ref accepts one
// where it accepts a name.
function declarationLabelTypes(active: boolean) {
  react.ref("trigger");
  react.ref(react.ref("boundary"), "bounded");
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

function boundedIdentityTypes(
  boundary: import("stillwind/react").StyleRef<"boundary">,
  identity: import("stillwind/react").StillwindDeclarationIdentity<"bounded">,
) {
  const bounded = react.ref(boundary, identity);
  react(bounded, bounded.anchor);
  svelte(bounded, bounded.positionAnchor);
  vue({ [bounded("hover")]: "text-accent" });
}
void boundedIdentityTypes;
