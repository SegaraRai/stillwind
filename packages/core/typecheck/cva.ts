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

const button = react.cva(
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
          ":": active ? "p-2" : "p-3",
        },
        large: "p-4",
      },
      disabled: {
        false: null,
        true: active && "opacity-50",
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
        className: "hover:shadow-md",
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

// @ts-expect-error CVA results cannot be composed through style().
react(button());
react.cva(null, {
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
react.cva(null, {
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
react.cva(null, {
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
react.cva(null, {
  // @ts-expect-error Class names cannot be selection axes.
  variants: {
    className: {
      small: "p-2",
    },
  },
});
react.cva({
  // @ts-expect-error CVA inputs require recognized rule keys.
  unknown: "ready",
});
react.cva({
  "hover:": {
    // @ts-expect-error Custom properties cannot occur under CSS variants.
    "--tone": "red",
  },
});
react.cva({
  "": {
    // @ts-expect-error Empty rule groups also exclude custom properties.
    "--tone": "red",
  },
});

react.cva([
  true,
  false,
  null,
  undefined,
  {
    "hover:": "shadow-md",
    "--tone": "red",
  },
]);
react.cva();

const classOnly: { class?: string; style?: Record<string, string> } = preact.cva("p-2")();
const solidProps: typeof classOnly = solid.cva("p-2")();
const vueProps: typeof classOnly = vue.cva("p-2")();
const stringStyle: { class?: string; style?: string } = svelte.cva({ "--tone": "red" })();
const astroProps: typeof stringStyle = astro.cva("p-2")();
void [classOnly, solidProps, vueProps, stringStyle, astroProps];
