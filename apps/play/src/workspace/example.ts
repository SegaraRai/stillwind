/** Files displayed and compiled when a playground workspace starts. */
export const exampleFiles = new Map<string, string>([
  [
    "/workspace/Component.tsx",
    `import { useState } from "react";
import { style } from "stillwind/react";

import { card } from "./styles";

export function App() {
  const [count, setCount] = useState(0);
  return <main {...style(card, "p-4 rounded-md")}>
    <h1>Stillwind Play</h1>
    <p>Edit this component or its imported styles.</p>
    <button {...style("bg-accent text-white px-4 py-2 rounded-md")} onClick={() => setCount(count + 1)}>
      Count: {count}
    </button>
  </main>;
}`,
  ],
  [
    "/workspace/styles.ts",
    `import { style } from "stillwind/react";

export const card = style("bg-surface");
`,
  ],
  [
    "/workspace/stillwind.css",
    `@import "stillwind";

@theme {
  --color-accent: oklch(.6 .2 250);
  --color-white: white;
  --color-surface: oklch(.97 .01 250);
  --spacing-2: .5rem;
  --spacing-4: 1rem;
  --radius-md: .5rem;
}`,
  ],
  [
    "/workspace/stillwind.config.ts",
    `import { defineConfig } from "stillwind/config";

export default defineConfig({
  projects: [
    {
      css: "stillwind.css",
      files: ["**/*.{ts,tsx}"],
      types: "stillwind.generated.d.ts",
    }
  ]
});
`,
  ],
  [
    "/workspace/tsconfig.json",
    JSON.stringify(
      {
        compilerOptions: {
          target: "ESNext",
          module: "ESNext",
          moduleResolution: "Bundler",
          jsx: "react-jsx",
          strict: true,
          noEmit: true,
          allowImportingTsExtensions: true,
          types: ["react", "react-dom"],
        },
        include: ["**/*.ts", "**/*.tsx"],
      },
      null,
      2,
    ),
  ],
]);
