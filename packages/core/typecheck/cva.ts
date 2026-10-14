import { style as astro } from "stillwind/astro";
import { style as preact } from "stillwind/preact";
import { style as react, type VariantProps } from "stillwind/react";
import { style as solid } from "stillwind/solid";
import { style as svelte } from "stillwind/svelte";
import { style as vue } from "stillwind/vue";

const button = react.cva(
  { "": "rounded", ":": { "hover:": "shadow-md" } },
  {
    variants: {
      size: { small: { ":": "p-2" }, large: "p-4" },
      disabled: { false: null, true: "opacity-50" },
      count: { 0: "p-0", 1: "p-1" },
    },
    defaultVariants: { size: "small", disabled: false, count: 0 },
    compoundVariants: [{ size: ["small", "large"], disabled: false, className: "hover:shadow-md" }],
  },
);

const selection: VariantProps<typeof button> = { size: null, disabled: true, count: 1 };
button(selection);
button({ size: undefined, class: ["external"], className: "extra" });
const props: { className?: string; style?: Record<string, string> } = button();
void props;

// @ts-expect-error Unknown option.
button({ size: "medium" });
// @ts-expect-error Boolean options require booleans.
button({ disabled: "true" });
// @ts-expect-error VariantProps excludes external class inputs.
const external: VariantProps<typeof button> = { className: "extra" };
void external;

// @ts-expect-error CVA results cannot be composed through style().
react(button());
// @ts-expect-error Unknown default option.
react.cva(null, { variants: { size: { small: "p-2" } }, defaultVariants: { size: "large" } });
react.cva(null, {
  variants: { size: { small: "p-2" } },
  // @ts-expect-error Unknown compound axis.
  compoundVariants: [{ other: "small", class: "p-2" }],
});
// @ts-expect-error Class names cannot be selection axes.
react.cva(null, { variants: { className: { small: "p-2" } } });
// @ts-expect-error Attributes are outside the CVA input subset.
react.cva({ "data-state": "ready" });
// @ts-expect-error Custom properties cannot occur under CSS variants.
react.cva({ "hover:": { "--tone": "red" } });
// @ts-expect-error Empty rule groups also exclude custom properties.
react.cva({ "": { "--tone": "red" } });

react.cva([true, false, null, undefined, { "hover:": "shadow-md", "--tone": "red" }]);
react.cva();

const classOnly: { class?: string; style?: Record<string, string> } = preact.cva("p-2")();
const solidProps: typeof classOnly = solid.cva("p-2")();
const vueProps: typeof classOnly = vue.cva("p-2")();
const stringStyle: { class?: string; style?: string } = svelte.cva({ "--tone": "red" })();
const astroProps: typeof stringStyle = astro.cva("p-2")();
void [classOnly, solidProps, vueProps, stringStyle, astroProps];
