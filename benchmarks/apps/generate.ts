/**
 * Generates React Router applications whose size and module graph shape can be varied.
 *
 * The `stillwind` variant imports `style()` from `stillwind/react`. The `plain` variant
 * imports a local `style()` that only joins class names and emits no CSS, so it measures
 * the same application without Stillwind. The `tailwind` variant uses the same local
 * `style()` and generates CSS for the class names with Tailwind CSS.
 */

/** Size and shape of a generated application. */
export interface AppShape {
  /** Number of route modules. Must be at least 1. */
  routes: number;
  /** Number of component modules. Must be at least `chain`. */
  components: number;
  /**
   * Length of each import chain between components. Each component imports the next one
   * in its chain and renders it. Its exported style composes the exported style of the
   * next one.
   */
  chain: number;
  /** Number of chain heads that each route imports. */
  componentsPerRoute: number;
  /** Whether modules import shared styles through `app/ui/index.ts`. */
  barrel: boolean;
}

export type AppVariant = "stillwind" | "plain" | "tailwind";

/** Absolute locations of the Tailwind CSS packages that the `tailwind` variant uses. */
export interface TailwindPaths {
  /** File URL of the `@tailwindcss/vite` entry module. */
  plugin: string;
  /** Absolute path of the stylesheet that `@import "tailwindcss"` loads. */
  stylesheet: string;
}

/** Edits applied by the harness. Each `from` occurs exactly once in `file`. */
export const appEdits = {
  leaf: {
    file: "app/components/c0.tsx",
    from: '"p-2 border-2"',
    to: '"p-3 border-2"',
    selector: "#hmr-leaf",
    property: "padding-top",
    value: "12px",
  },
  shared: {
    file: "app/ui/base.tsx",
    from: '"p-2 border-4"',
    to: '"p-3 border-4"',
    selector: "#hmr-shared",
    property: "padding-top",
    value: "12px",
  },
  theme: {
    file: "app/theme.css",
    from: "rgb(10, 20, 30)",
    to: "rgb(40, 50, 60)",
    selector: "#hmr-theme",
    property: "color",
    value: "rgb(40, 50, 60)",
  },
} as const;

export type AppEditName = keyof typeof appEdits;

/**
 * Builds the files of one application.
 *
 * @param variant - Which styling setup the application uses.
 * @param shape - Application size and graph shape.
 * @param tailwind - Package locations, required for the `tailwind` variant. The generated
 * application lives outside the benchmark package, so it cannot resolve them by name.
 * @returns File contents keyed by POSIX paths relative to the application root.
 */
export function generateApp(
  variant: AppVariant,
  shape: AppShape,
  tailwind?: TailwindPaths,
): Map<string, string> {
  if (shape.routes < 1 || shape.components < shape.chain || shape.chain < 1) {
    throw new Error(`Invalid application shape: ${JSON.stringify(shape)}`);
  }
  if (variant === "tailwind" && !tailwind) {
    throw new Error("The tailwind variant requires Tailwind CSS package locations");
  }
  const files = new Map<string, string>();
  const styleModule = (depth: number) =>
    variant === "stillwind" ? "stillwind/react" : `${depth === 0 ? "./" : "../"}plain-style`;
  const uiModule = shape.barrel ? "../ui" : "../ui/base";
  const chains = Math.ceil(shape.components / shape.chain);

  const plugins = {
    stillwind: [
      'import { stillwind } from "@stillwind/integration/vite";',
      'stillwind({ css: "app/theme.css" })',
    ],
    plain: undefined,
    tailwind: tailwind && [
      `import tailwindcss from ${JSON.stringify(tailwind.plugin)};`,
      "tailwindcss()",
    ],
  }[variant];
  files.set(
    "vite.config.ts",
    `import { reactRouter } from "@react-router/dev/vite";
${plugins ? `${plugins[0]}\n` : ""}import { defineConfig } from "vite";

export default defineConfig({
  plugins: [${plugins ? `${plugins[1]}, ` : ""}reactRouter()],
});
`,
  );
  files.set("react-router.config.ts", "export default { ssr: true };\n");
  files.set(
    "tsconfig.json",
    JSON.stringify(
      {
        compilerOptions: {
          target: "ES2022",
          module: "ESNext",
          moduleResolution: "bundler",
          jsx: "react-jsx",
          strict: true,
          skipLibCheck: true,
          noEmit: true,
          types: [],
        },
        include: ["app/**/*.ts", "app/**/*.tsx"],
      },
      null,
      2,
    ) + "\n",
  );
  files.set(
    "app/theme.css",
    `@import ${JSON.stringify(variant === "tailwind" ? tailwind!.stylesheet : "stillwind")};

@theme {
  --color-brand: rgb(10, 20, 30);
}
`,
  );
  if (variant !== "stillwind") {
    files.set("app/plain-style.ts", plainStyle);
  }

  files.set(
    "app/ui/base.tsx",
    `import { style } from "${styleModule(1)}";

export const button = style({
  "": "inline-flex items-center gap-1 px-2 py-1 rounded-md border border-gray-300 bg-white",
  "hover:": "bg-gray-100",
  "disabled:": "opacity-50 cursor-default",
});
export const primaryButton = style(button, "bg-blue-600 text-white border-blue-600", {
  "hover:": "bg-blue-700",
});
export const dangerButton = style(button, "text-red-600");
export const card = style("flex flex-col gap-2 p-4 rounded-lg border border-gray-200 bg-white");
export const elevatedCard = style(card, "shadow-md");
export const stack = style("flex flex-col gap-1");
export const row = style("flex flex-row items-center gap-2");
export const heading = style("text-lg font-semibold text-gray-900");
export const caption = style("text-xs text-gray-500");
export const hmrShared = style("p-2 border-4");
`,
  );
  if (shape.barrel) {
    files.set("app/ui/index.ts", 'export * from "./base";\n');
  }

  for (let i = 0; i < shape.components; i++) {
    const next = (i + 1) % shape.chain !== 0 && i + 1 < shape.components ? i + 1 : undefined;
    const base = ["card", "elevatedCard", "stack", "row"][i % 4];
    files.set(
      `app/components/c${i}.tsx`,
      `import { style } from "${styleModule(1)}";
import { ${base}, button, caption, primaryButton } from "${uiModule}";
${next === undefined ? "" : `import { C${next}, c${next}Frame } from "./c${next}";\n`}
export const c${i}Frame = style(${next === undefined ? base : `c${next}Frame`}, "p-${(i % 4) + 1}");
const frame = style(c${i}Frame, "gap-2", {
  "hover:": "bg-gray-50",
});
${i === 0 ? 'const hmrLeaf = style("p-2 border-2");\n' : ""}
export function C${i}({ active = false }: { active?: boolean }) {
  return (
    <section {...style(frame)}>
      <h3 {...style("text-sm font-semibold", active && "text-blue-700")}>Component ${i}</h3>
      <p {...style(caption, "mt-${i % 6}")}>Generated component ${i}</p>
      <button {...style(active ? primaryButton : button)}>Action</button>
${i === 0 ? '      <div id="hmr-leaf" {...style(hmrLeaf)}>leaf</div>\n' : ""}${next === undefined ? "" : `      <C${next} active={!active} />\n`}    </section>
  );
}
`,
    );
  }

  const routes: string[] = [];
  for (let r = 0; r < shape.routes; r++) {
    const heads = new Set<number>();
    if (r === 0) {
      heads.add(0);
    }
    for (let k = 0; heads.size < Math.min(shape.componentsPerRoute, chains); k++) {
      heads.add(((r * 7 + k * 13) % chains) * shape.chain);
    }
    const imports = [...heads].map((head) => `import { C${head} } from "../components/c${head}";`);
    files.set(
      `app/routes/r${r}.tsx`,
      `import { Link } from "react-router";
import { style } from "${styleModule(1)}";
import { heading, hmrShared, stack } from "${uiModule}";
${imports.join("\n")}

const layout = style(stack, "max-w-3xl mx-auto p-6 gap-4");

export default function Route${r}() {
  return (
    <main {...style(layout)}>
      <h1 {...style(heading)}>Route ${r}</h1>
      <p id="hmr-theme" {...style("text-xs text-brand")}>theme</p>
      <div id="hmr-shared" {...style(hmrShared)}>shared</div>
      <nav {...style("flex gap-2")}>
        <Link to="/r${(r + 1) % shape.routes}">Next</Link>
      </nav>
${[...heads].map((head) => `      <C${head} />`).join("\n")}
    </main>
  );
}
`,
    );
    routes.push(r === 0 ? `index("routes/r0.tsx")` : `route("r${r}", "routes/r${r}.tsx")`);
  }
  files.set(
    "app/routes.ts",
    `import { index, route } from "@react-router/dev/routes";

export default [
${routes.map((route) => `  ${route},`).join("\n")}
];
`,
  );
  files.set(
    "app/root.tsx",
    `${variant === "tailwind" ? 'import "./theme.css";\n' : ""}import { useEffect } from "react";
import { Links, Meta, Outlet, Scripts, ScrollRestoration } from "react-router";

export default function Root() {
  useEffect(() => {
    document.documentElement.dataset.hydrated = "yes";
  }, []);
  return (
    <html lang="en">
      <head>
        <link rel="icon" href="data:," />
        <Meta />
        <Links />
      </head>
      <body>
        <Outlet />
        <ScrollRestoration />
        <Scripts />
      </body>
    </html>
  );
}
`,
  );
  return files;
}

/** Local `style()` replacement that only joins authored classes. */
const plainStyle = `type Input =
  | string
  | false
  | null
  | undefined
  | { className?: string }
  | Record<string, string>;

export function style(...inputs: Input[]): { className: string } {
  const classes: string[] = [];
  for (const input of inputs) {
    if (!input) {
      continue;
    }
    if (typeof input === "string") {
      classes.push(input);
    } else if (typeof input.className === "string") {
      classes.push(input.className);
    } else {
      for (const [prefix, value] of Object.entries(input)) {
        classes.push(...value.split(" ").map((name) => prefix + name));
      }
    }
  }
  return { className: classes.join(" ") };
}
`;
