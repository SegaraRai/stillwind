/** Each project and the computed styles that its `#target` must have in every build. */
export const projectCases: [string, Record<string, string>][] = [
  ["forms", { appearance: "none", "background-color": "rgb(255, 255, 255)" }],
  ["scrollbar", { "scrollbar-width": "thin", "--scrollbar-thumb": "#123456" }],
  ["animate", { "animation-duration": "0.3s", "--tw-enter-opacity": "0.5" }],
  ["motion", { "--motion-duration": "700ms" }],
  ["radix", { "background-color": "rgb(18, 52, 86)" }],
  ["react-aria", { "background-color": "rgb(18, 52, 86)" }],
  ["headless", { "background-color": "rgb(18, 52, 86)" }],
  ["iconify", { display: "inline-block", width: "24px" }],
  ["safe-area", { "padding-top": "16px" }],
  ["scrollbar-hide", { "scrollbar-width": "none" }],
  ["animated", { "animation-duration": "0.7s" }],
  ["tw-animate", { "animation-duration": "0.3s", "--tw-enter-opacity": ".5" }],
  // The article's color reaches `#target`, but `not-prose` removes the paragraph margin.
  ["typography", { color: "oklch(0.373 0.034 259.733)", "margin-top": "0px" }],
  // `join` removes the end radius of its first item, which keeps the primary color.
  ["daisyui", { "background-color": "oklch(0.45 0.24 277.023)", "border-start-end-radius": "0px" }],
  // daisyUI's own variables and the prefixed theme variables stay consistent.
  [
    "theme-prefix",
    {
      "background-color": "oklch(0.45 0.24 277.023)",
      "--tw-color-brand": "#123456",
      "--color-brand": "",
    },
  ],
  ["preline", { "background-color": "rgb(18, 52, 86)" }],
  ["intersect", { "background-color": "rgb(18, 52, 86)" }],
];
