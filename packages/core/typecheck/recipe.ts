import { style as astro } from "stillwind/astro";
import { style as preact } from "stillwind/preact";
import { style as react, type VariantProps } from "stillwind/react";
import { style as solid } from "stillwind/solid";
import { style as svelte } from "stillwind/svelte";
import { style as vue } from "stillwind/vue";

type Assert<T extends true> = T;

type Equal<A, B> =
  (<T>() => T extends A ? 1 : 2) extends <T>() => T extends B ? 1 : 2 ? true : false;

declare const active: boolean;

const button = react.recipe(
  {
    "": "rounded",
    ":": {
      "hover:": "shadow-md",
    },
  },
  {
    variants: {
      size: {
        small: {
          ":": ["p-2", { "focus:": "p-3" }],
        },
        large: "p-4",
      },
      disabled: {
        false: null,
        true: "opacity-50",
      },
      count: {
        0: "p-0",
        1: "p-1",
      },
    },
    defaultVariants: {
      size: "small",
      disabled: false,
      count: 0,
    },
    compoundVariants: [
      {
        size: ["small", "large"],
        disabled: false,
        count: [0, 1],
        className: { "hover:": ["shadow-md", { "focus:": "underline" }] },
      },
    ],
  },
);

const selection: VariantProps<typeof button> = {
  size: null,
  disabled: true,
  count: 1,
};

export type InferredSelection = Assert<
  Equal<
    VariantProps<typeof button>,
    {
      readonly size?: "small" | "large" | null | undefined;
      readonly disabled?: boolean | null | undefined;
      readonly count?: 0 | 1 | null | undefined;
    }
  >
>;

button(selection);
button({ disabled: active });
button({
  size: undefined,
  class: ["external"],
  className: "extra",
});
const props: {
  className?: string;
  style?: Record<string, string>;
} = button();
void props;

button({
  // @ts-expect-error Unknown option.
  size: "medium",
});
button({
  // @ts-expect-error Boolean options require booleans.
  disabled: "true",
});
const external: VariantProps<typeof button> = {
  // @ts-expect-error VariantProps excludes external class inputs.
  className: "extra",
};
void external;

// @ts-expect-error recipe results cannot be composed through style().
react(button());
react.recipe(null, {
  variants: {
    size: {
      small: "p-2",
    },
  },
  defaultVariants: {
    // @ts-expect-error Unknown default option.
    size: "large",
  },
});
react.recipe(null, {
  variants: {
    size: {
      small: "p-2",
    },
  },
  compoundVariants: [
    {
      // @ts-expect-error Compound arrays require declared options.
      size: ["large"],
      class: "p-2",
    },
  ],
});
react.recipe(null, {
  variants: {
    size: {
      small: "p-2",
    },
  },
  compoundVariants: [
    {
      // @ts-expect-error Unknown compound axis.
      other: "small",
      class: "p-2",
    },
  ],
});
react.recipe(null, {
  // @ts-expect-error Class names cannot be selection axes.
  variants: {
    className: {
      small: "p-2",
    },
  },
});
react.recipe({
  // @ts-expect-error recipe inputs require recognized rule keys.
  unknown: "ready",
});
react.recipe({
  "hover:": {
    // @ts-expect-error Custom properties cannot occur under CSS variants.
    "--tone": "red",
  },
});
react.recipe({
  "": {
    // @ts-expect-error Empty rule groups also exclude custom properties.
    "--tone": "red",
  },
});

react.recipe([
  true,
  false,
  null,
  undefined,
  {
    "hover:": "shadow-md",
    "--tone": active ? "red" : "pink",
  },
]);
react.recipe();

const classOnly: { class?: string; style?: Record<string, string> } = preact.recipe("p-2")();
const solidProps: typeof classOnly = solid.recipe("p-2")();
const vueProps: typeof classOnly = vue.recipe("p-2")();
const stringStyle: { class?: string; style?: string } = svelte.recipe({ "--tone": "red" })();
const astroProps: typeof stringStyle = astro.recipe("p-2")();
void [classOnly, solidProps, vueProps, stringStyle, astroProps];

const card = react.recipe("rounded", {
  slots: {
    icon: "size-4",
    label: "text-sm",
  },
  variants: {
    size: {
      small: {
        base: "p-2",
        icon: "size-3",
      },
      large: "p-4",
    },
  },
  defaultVariants: {
    size: "small",
  },
  compoundVariants: [
    {
      size: "large",
      class: {
        label: "text-lg",
      },
    },
  ],
  compoundSlots: [
    {
      slots: ["icon", "label"],
      size: "small",
      class: "gap-1",
    },
  ],
});
const parts = card({
  size: "large",
  className: "outer",
});
const slotProps: typeof props = parts.icon({
  size: "small",
  className: "own",
});
void [slotProps, parts.base(), parts.label()];

export type InferredSlotSelection = Assert<
  Equal<
    VariantProps<typeof card>,
    {
      readonly size?: "small" | "large" | null | undefined;
    }
  >
>;

// @ts-expect-error Unknown slot.
void card().missing;
// @ts-expect-error Compound slots list declared slots only.
react.recipe(null, {
  slots: { icon: "size-4" },
  compoundSlots: [
    {
      slots: ["label"],
      class: "gap-1",
    },
  ],
});
