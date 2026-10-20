import { style } from "stillwind/svelte";

export function createTone(tone: string) {
  return style.cva(
    [
      "text-[var(--tone)]",
      {
        "--tone": tone,
      },
    ],
    {
      variants: {
        kind: {
          inherited: true,
          fixed: {
            "--tone": "red",
          },
          emphasized: ["font-bold", { "hover:": "underline" }],
        },
      },
      defaultVariants: {
        kind: "inherited",
      },
      compoundVariants: [
        {
          kind: "fixed",
          class: { "--tone": "blue" },
        },
      ],
    },
  );
}
