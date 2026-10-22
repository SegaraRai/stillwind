export const featureTopics = [
  {
    id: "syntax",
    name: "Syntax",
    frames: [
      {
        label: "Utility strings",
        source: 'style("rounded-control px-4 py-2")',
        focus: '"rounded-control px-4 py-2"',
        description: "Write a utility string with the Tailwind CSS classes you know.",
      },
      {
        label: "JavaScript conditions",
        source: 'style("rounded-control px-4 py-2", emphasized && "shadow-sm")',
        focus: 'emphasized && "shadow-sm"',
        description:
          "Combine utilities with JavaScript conditions. A false condition contributes no classes.",
      },
      {
        label: "Arrays",
        source: 'style(["rounded-control px-4 py-2", emphasized && "shadow-sm"])',
        focus: '["rounded-control px-4 py-2", emphasized && "shadow-sm"]',
        description:
          "Compose strings and conditional values in arrays. Keep related inputs together.",
      },
      {
        label: "CSS states",
        source: [
          "style(",
          '  ["rounded-control px-4 py-2", emphasized && "shadow-sm"],',
          '  { "aria-pressed:": "bg-action text-action-foreground" },',
          ")",
        ].join("\n"),
        focus: '"aria-pressed:": "bg-action text-action-foreground"',
        description:
          "Use an attribute to select CSS rules. Toggling aria-pressed updates the button while its class name stays the same.",
      },
      {
        label: "Variant groups",
        source: [
          "style(",
          '  ["rounded-control px-4 py-2", emphasized && "shadow-sm"],',
          "  {",
          '    "aria-pressed:": ["bg-action text-action-foreground", "font-medium"],',
          "  },",
          ")",
        ].join("\n"),
        focus: '["bg-action text-action-foreground", "font-medium"]',
        description:
          "Group strings and arrays under one variant. The aria-pressed state applies the grouped rules together.",
      },
      {
        label: "Nested variants",
        source: [
          "style(",
          '  ["rounded-control px-4 py-2", emphasized && "shadow-sm"],',
          "  {",
          '    "aria-pressed:": [',
          '      "bg-action text-action-foreground font-medium",',
          '      { "hover:": "bg-action-hover" },',
          "    ],",
          "  },",
          ")",
        ].join("\n"),
        focus: '{ "hover:": "bg-action-hover" }',
        description:
          "Nest variants to describe conditions that apply together. Hover styles only apply while the button is pressed.",
      },
      {
        label: "CSS custom properties",
        source: [
          "style(",
          '  ["rounded-control px-4 py-2", emphasized && "shadow-sm"],',
          "  {",
          '    "--accent": accent,',
          '    "aria-pressed:": [',
          '      "bg-(--accent) text-action-foreground font-medium",',
          '      { "hover:": "brightness-110" },',
          "    ],",
          "  },",
          ")",
        ].join("\n"),
        focus: '"--accent": accent',
        description:
          "Pass a value through a CSS custom property and reference it from utilities. CSS can use the value across different rules.",
      },
    ],
  },
  {
    id: "relationships",
    name: "Relationships",
    frames: [
      {
        label: "Container queries",
        source: [
          "const box = style.ref();",
          "",
          'style(box, box.asContainer("inline-size"));',
          "",
          'style("grid gap-4 justify-items-center text-center", {',
          '  [box.at("width >= 13rem")]: [',
          '    "grid-cols-[3rem_1fr]",',
          '    "justify-items-start text-left",',
          "  ],",
          "});",
        ].join("\n"),
        focus: '[box.at("width >= 13rem")]',
        description:
          "Name a container with a ref and query its width from an article card. Its thumbnail and text switch from a row to a stack as the container narrows.",
      },
      {
        label: "Scoped refs",
        source: [
          "const card = style.ref();",
          "const button = style.ref();",
          "",
          "style(card);",
          "style(button);",
          'style("opacity-0", {',
          '  [button("aria-pressed").in(card)]:',
          '    "opacity-100",',
          "});",
        ].join("\n"),
        focus: 'button("aria-pressed").in(card)',
        description:
          "Scope a Save button's pressed state to its article card. Each button reveals the Saved label in its own card.",
      },
      {
        label: "CSS Anchor Positioning",
        source: [
          "const saveButton = style.ref();",
          "",
          "style(saveButton, saveButton.asAnchor());",
          "style(",
          "  saveButton.positionAnchor(),",
          '  "absolute [position-area:bottom]",',
          '  "[position-try-fallbacks:flip-block]",',
          ");",
        ].join("\n"),
        focus: [
          "saveButton.positionAnchor(),",
          '  "absolute [position-area:bottom]",',
          '  "[position-try-fallbacks:flip-block]"',
        ].join("\n"),
        description:
          "Declare a CSS anchor on a Save button. Its attached label shifts sideways and flips above the button to stay inside the container.",
      },
    ],
  },
  {
    id: "motion",
    name: "Motion",
    frames: [
      {
        label: "Keyframes",
        source: [
          "const enter = style.keyframes({",
          '  from: "opacity-0 translate-y-8",',
          '  to: "opacity-100 translate-y-0",',
          "});",
          "",
          'style(enter("duration-420 ease-out both"));',
        ].join("\n"),
        focus: ['from: "opacity-0 translate-y-8",', '  to: "opacity-100 translate-y-0",'].join(
          "\n",
        ),
        description:
          "Declare reusable keyframes with utility strings. Apply duration, easing, and fill options where you use the animation.",
      },
      {
        label: "View Transitions",
        source: [
          "const article = style.viewTransition({",
          '  group: "duration-300 ease-out",',
          '  old: "duration-150",',
          '  new: "delay-150 duration-150",',
          "});",
          "",
          'style(article.for("thumbnail"));',
          'style(article.for("title"));',
          'style(article.for("meta"));',
        ].join("\n"),
        focus: [
          'group: "duration-300 ease-out",',
          '  old: "duration-150",',
          '  new: "delay-150 duration-150",',
        ].join("\n"),
        description:
          "Pair an article's thumbnail, title, and metadata across its card and reading view. Timing utilities set each snapshot's animation, so the new snapshot fades in after the old one fades out. Call document.startViewTransition() to animate the view change.",
      },
    ],
  },
] as const;
