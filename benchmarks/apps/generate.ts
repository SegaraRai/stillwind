/**
 * Generates applications of several frameworks whose size and module graph shape can be varied.
 *
 * The `stillwind` variant imports `style()` from the Stillwind entry of the framework. The `plain`
 * variant imports a local `style()` that only joins class names and emits no CSS, so it measures
 * the same application without Stillwind. The `tailwind` variant uses the same local `style()` and
 * generates CSS for the class names with Tailwind CSS. Only React Router applications have the
 * `tailwind` variant.
 *
 * Every framework generates the same module graph: shared styles in `ui/base.ts`, chains of
 * components whose exported styles compose the next component's exported style, and routes that
 * render chain heads. Astro components cannot export values, so each Astro component declares its
 * exported style in a sibling `.frame.ts` module.
 */
import { posix } from "node:path";

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
  /** Whether modules import shared styles through `ui/index.ts`. */
  barrel: boolean;
}

export type AppVariant = "stillwind" | "plain" | "tailwind";

/**
 * Framework of a generated application. `vue`, `svelte`, and `solid` are Vite single-page
 * applications that load the route module that matches the location.
 */
export type AppFramework =
  | "react-router"
  | "vue"
  | "svelte"
  | "solid"
  | "astro"
  | "next"
  | "sveltekit";

/** Absolute locations of the Tailwind CSS packages that the `tailwind` variant uses. */
export interface TailwindPaths {
  /** File URL of the `@tailwindcss/vite` entry module. */
  plugin: string;
  /** Absolute path of the stylesheet that `@import "tailwindcss"` loads. */
  stylesheet: string;
}

/** One edit that the harness applies. `from` occurs exactly once in `file`. */
export interface AppEdit {
  /** POSIX path relative to the application root. */
  file: string;
  from: string;
  to: string;
  /** Element whose computed style shows the edit. */
  selector: string;
  property: string;
  /** Computed value of `property` after the edit. */
  value: string;
}

export type AppEditName = "leaf" | "shared" | "theme";

/** Settings that differ between frameworks. */
interface Dialect {
  /** Directory that contains the application sources. */
  source: string;
  /** Extension of component and route modules. */
  extension: string;
  /** Stillwind entry that exports `style()`. */
  entry: string;
  /** Property of the object that `style()` returns. */
  classProperty: "className" | "class";
}

const dialects: Record<AppFramework, Dialect> = {
  "react-router": { source: "app", extension: "tsx", entry: "react", classProperty: "className" },
  vue: { source: "src", extension: "vue", entry: "vue", classProperty: "class" },
  svelte: { source: "src", extension: "svelte", entry: "svelte", classProperty: "class" },
  solid: { source: "src", extension: "tsx", entry: "solid", classProperty: "class" },
  astro: { source: "src", extension: "astro", entry: "astro", classProperty: "class" },
  next: { source: "app", extension: "tsx", entry: "react", classProperty: "className" },
  sveltekit: { source: "src", extension: "svelte", entry: "svelte", classProperty: "class" },
};

/**
 * Lists the edits of an application.
 *
 * @param framework - Framework whose file layout the edits use.
 * @returns A leaf component edit, a shared style edit, and a theme edit.
 */
export function appEdits(framework: AppFramework): Record<AppEditName, AppEdit> {
  const { source, extension } = dialects[framework];
  return {
    leaf: {
      file: `${source}/components/c0.${extension}`,
      from: '"p-2 border-2"',
      to: '"p-3 border-2"',
      selector: "#hmr-leaf",
      property: "padding-top",
      value: "12px",
    },
    shared: {
      file: `${source}/ui/base.ts`,
      from: '"p-2 border-4"',
      to: '"p-3 border-4"',
      selector: "#hmr-shared",
      property: "padding-top",
      value: "12px",
    },
    theme: {
      file: `${source}/theme.css`,
      from: "rgb(10, 20, 30)",
      to: "rgb(40, 50, 60)",
      selector: "#hmr-theme",
      property: "color",
      value: "rgb(40, 50, 60)",
    },
  };
}

/** One component of a chain. */
interface ComponentPlan {
  index: number;
  /** Next component of the chain, which this component imports and renders. */
  next?: number;
  /** Shared style that the last component of a chain composes. */
  base: string;
}

/** One route and the chain heads that it renders. */
interface RoutePlan {
  index: number;
  heads: number[];
  /** Index of the route that the route links to. */
  link: number;
}

/** Writes the files of one application. */
interface Writer {
  framework: AppFramework;
  variant: AppVariant;
  /** Hash digits of generated names, or undefined for the compiler default. */
  hashLength?: number;
  dialect: Dialect;
  shape: AppShape;
  files: Map<string, string>;
  /** Returns the module request for `target`, relative to `file` unless `target` is a package. */
  request: (file: string, target: string) => string;
  /** Module request of `style()` from `file`. */
  style: (file: string) => string;
  /** Module request of the shared styles from `file`. */
  ui: (file: string) => string;
}

/**
 * Builds the files of one application.
 *
 * @param framework - Framework of the application.
 * @param variant - Which styling setup the application uses.
 * @param shape - Application size and graph shape.
 * @param tailwind - Package locations, required for the `tailwind` variant. The generated
 * application lives outside the benchmark package, so it cannot resolve them by name.
 * @param hashLength - Hash digits of generated names in the `stillwind` variant. Omission keeps
 * the compiler default.
 * @returns File contents keyed by POSIX paths relative to the application root.
 * @throws If the shape is invalid, or the framework has no `tailwind` variant.
 */
export function generateApp(
  framework: AppFramework,
  variant: AppVariant,
  shape: AppShape,
  tailwind?: TailwindPaths,
  hashLength?: number,
): Map<string, string> {
  if (shape.routes < 1 || shape.components < shape.chain || shape.chain < 1) {
    throw new Error(`Invalid application shape: ${JSON.stringify(shape)}`);
  }
  if (variant === "tailwind" && (framework !== "react-router" || !tailwind)) {
    throw new Error(
      "The tailwind variant requires React Router and Tailwind CSS package locations",
    );
  }
  const dialect = dialects[framework];
  const request = (file: string, target: string) => {
    if (!target.startsWith(".")) {
      return target;
    }
    const relative = posix.relative(posix.dirname(file), target);
    return relative.startsWith(".") ? relative : `./${relative}`;
  };
  const writer: Writer = {
    framework,
    variant,
    hashLength,
    dialect,
    shape,
    files: new Map(),
    request,
    style: (file) =>
      variant === "stillwind"
        ? `stillwind/${dialect.entry}`
        : request(file, `./${dialect.source}/plain-style`),
    ui: (file) =>
      request(file, shape.barrel ? `./${dialect.source}/ui` : `./${dialect.source}/ui/base`),
  };
  const { files, style } = writer;
  const { source } = dialect;

  files.set(
    `${source}/theme.css`,
    `@import ${JSON.stringify(variant === "tailwind" ? tailwind!.stylesheet : "stillwind")};

@theme {
  --color-brand: rgb(10, 20, 30);
}
`,
  );
  if (variant === "stillwind" && framework !== "react-router") {
    files.set(
      "stillwind.config.ts",
      `import { defineConfig } from "stillwind/config";

export default defineConfig({
  projects: [
    {
      css: "${source}/theme.css",${
        hashLength === undefined
          ? ""
          : `
      hashLength: ${hashLength},`
      }
      files: ["${source}/**/*.{ts,tsx,vue,svelte,astro}"],
    },
  ],
});
`,
    );
  }
  if (variant !== "stillwind") {
    files.set(`${source}/plain-style.ts`, plainStyle(dialect.classProperty));
  }
  const base = `${source}/ui/base.ts`;
  files.set(
    base,
    `import { style } from "${style(base)}";

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
    files.set(`${source}/ui/index.ts`, 'export * from "./base";\n');
  }

  const components: ComponentPlan[] = [];
  for (let index = 0; index < shape.components; index++) {
    components.push({
      index,
      next: (index + 1) % shape.chain !== 0 && index + 1 < shape.components ? index + 1 : undefined,
      base: ["card", "elevatedCard", "stack", "row"][index % 4],
    });
  }
  const chains = Math.ceil(shape.components / shape.chain);
  const routes: RoutePlan[] = [];
  for (let index = 0; index < shape.routes; index++) {
    const heads = new Set<number>();
    if (index === 0) {
      heads.add(0);
    }
    for (let k = 0; heads.size < Math.min(shape.componentsPerRoute, chains); k++) {
      heads.add(((index * 7 + k * 13) % chains) * shape.chain);
    }
    routes.push({ index, heads: [...heads], link: (index + 1) % shape.routes });
  }

  const generators: Record<
    AppFramework,
    (writer: Writer, components: ComponentPlan[], routes: RoutePlan[]) => void
  > = {
    "react-router": (writer, components, routes) =>
      reactRouter(writer, components, routes, tailwind),
    vue: (writer, components, routes) => singlePage(writer, components, routes, vue),
    svelte: (writer, components, routes) => singlePage(writer, components, routes, svelte),
    solid: (writer, components, routes) => singlePage(writer, components, routes, solid),
    astro,
    next,
    sveltekit,
  };
  generators[framework](writer, components, routes);
  return files;
}

/** Local `style()` replacement that only joins authored classes. */
function plainStyle(property: Dialect["classProperty"]): string {
  return `type Input =
  | string
  | false
  | null
  | undefined
  | { ${property}?: string }
  | Record<string, string>;

export function style(...inputs: Input[]): { ${property}: string } {
  const classes: string[] = [];
  for (const input of inputs) {
    if (!input) {
      continue;
    }
    if (typeof input === "string") {
      classes.push(input);
    } else if (typeof input.${property} === "string") {
      classes.push(input.${property});
    } else {
      for (const [prefix, value] of Object.entries(input)) {
        classes.push(...value.split(" ").map((name) => prefix + name));
      }
    }
  }
  return { ${property}: classes.join(" ") };
}
`;
}

/** Writes a TypeScript configuration with the given JSX settings. */
function typescriptConfig(writer: Writer, compilerOptions: Record<string, unknown>) {
  writer.files.set(
    "tsconfig.json",
    JSON.stringify(
      {
        compilerOptions: {
          target: "ES2022",
          module: "ESNext",
          moduleResolution: "bundler",
          strict: true,
          skipLibCheck: true,
          noEmit: true,
          types: [],
          ...compilerOptions,
        },
        include: [`${writer.dialect.source}/**/*`],
      },
      null,
      2,
    ) + "\n",
  );
}

/** Returns the Vite plugin import and call of the variant, or undefined without Stillwind. */
function stillwindPlugin(writer: Writer, options = ""): [string, string] | undefined {
  return writer.variant === "stillwind"
    ? ['import { stillwind } from "@stillwind/integration/vite";', `stillwind(${options})`]
    : undefined;
}

/** Writes a Vite configuration with the Stillwind plugin of the variant before `plugins`. */
function viteConfig(
  writer: Writer,
  imports: string[],
  plugins: string[],
  stillwind = stillwindPlugin(writer),
) {
  writer.files.set(
    "vite.config.ts",
    `${[...(stillwind ? [stillwind[0]] : []), ...imports].join("\n")}
import { defineConfig } from "vite";

export default defineConfig({
  plugins: [${[...(stillwind ? [stillwind[1]] : []), ...plugins].join(", ")}],
});
`,
  );
}

/** Writes JSX components that render like React components. */
function jsxComponents(
  writer: Writer,
  components: ComponentPlan[],
  active: (component: string) => string,
) {
  const { source, extension } = writer.dialect;
  for (const { index, next, base } of components) {
    const file = `${source}/components/c${index}.${extension}`;
    const value = active("props");
    writer.files.set(
      file,
      `import { style } from "${writer.style(file)}";
import { ${base}, button, caption, primaryButton } from "${writer.ui(file)}";
${next === undefined ? "" : `import { C${next}, c${next}Frame } from "./c${next}";\n`}
export const c${index}Frame = style(${next === undefined ? base : `c${next}Frame`}, "p-${(index % 4) + 1}");
const frame = style(c${index}Frame, "gap-2", {
  "hover:": "bg-gray-50",
});
${index === 0 ? 'const hmrLeaf = style("p-2 border-2");\n' : ""}
export function C${index}(props: { active?: boolean }) {
  return (
    <section {...style(frame)}>
      <h3 {...style("text-sm font-semibold", ${value} && "text-blue-700")}>Component ${index}</h3>
      <p {...style(caption, "mt-${index % 6}")}>Generated component ${index}</p>
      <button {...style(${value} ? primaryButton : button)}>Action</button>
${index === 0 ? '      <div id="hmr-leaf" {...style(hmrLeaf)}>leaf</div>\n' : ""}${next === undefined ? "" : `      <C${next} active={!${value}} />\n`}    </section>
  );
}
`,
    );
  }
}

/**
 * Returns the body of a JSX route.
 *
 * @param link - Element that links to the next route.
 */
function jsxRoute(writer: Writer, file: string, route: RoutePlan, imports: string[], link: string) {
  return `${imports.join("\n")}${imports.length ? "\n" : ""}import { style } from "${writer.style(file)}";
import { heading, hmrShared, stack } from "${writer.ui(file)}";
${route.heads.map((head) => `import { C${head} } from "${writer.request(file, `./${writer.dialect.source}/components/c${head}`)}";`).join("\n")}

const layout = style(stack, "max-w-3xl mx-auto p-6 gap-4");

export default function Route${route.index}() {
  return (
    <main {...style(layout)}>
      <h1 {...style(heading)}>Route ${route.index}</h1>
      <p id="hmr-theme" {...style("text-xs text-brand")}>theme</p>
      <div id="hmr-shared" {...style(hmrShared)}>shared</div>
      <nav {...style("flex gap-2")}>
        ${link}
      </nav>
${route.heads.map((head) => `      <C${head} />`).join("\n")}
    </main>
  );
}
`;
}

function reactRouter(
  writer: Writer,
  components: ComponentPlan[],
  routes: RoutePlan[],
  tailwind?: TailwindPaths,
) {
  const { files, variant } = writer;
  viteConfig(
    writer,
    ['import { reactRouter } from "@react-router/dev/vite";'],
    ["reactRouter()"],
    variant === "tailwind"
      ? [`import tailwindcss from ${JSON.stringify(tailwind!.plugin)};`, "tailwindcss()"]
      : stillwindPlugin(
          writer,
          `{ css: "app/theme.css"${writer.hashLength === undefined ? "" : `, hashLength: ${writer.hashLength}`} }`,
        ),
  );
  files.set("react-router.config.ts", "export default { ssr: true };\n");
  typescriptConfig(writer, { jsx: "react-jsx" });
  jsxComponents(writer, components, (props) => `(${props}.active ?? false)`);
  for (const route of routes) {
    const file = `app/routes/r${route.index}.tsx`;
    files.set(
      file,
      jsxRoute(
        writer,
        file,
        route,
        ['import { Link } from "react-router";'],
        `<Link to="/r${route.link}">Next</Link>`,
      ),
    );
  }
  files.set(
    "app/routes.ts",
    `import { index, route } from "@react-router/dev/routes";

export default [
${routes.map((route) => (route.index === 0 ? '  index("routes/r0.tsx"),' : `  route("r${route.index}", "routes/r${route.index}.tsx"),`)).join("\n")}
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
}

/** Writes the components and routes of a single-page framework. */
interface SinglePage {
  /** Vite plugin import and call. */
  plugin: [string, string];
  /** TypeScript JSX settings. */
  compilerOptions: Record<string, unknown>;
  /** Statements of `main.ts` that render `Route` into `target` once loaded. */
  mount: [string, string];
  components(writer: Writer, components: ComponentPlan[]): void;
  route(writer: Writer, file: string, route: RoutePlan): string;
}

function singlePage(
  writer: Writer,
  components: ComponentPlan[],
  routes: RoutePlan[],
  framework: SinglePage,
) {
  const { files } = writer;
  const { extension } = writer.dialect;
  viteConfig(writer, [framework.plugin[0]], [framework.plugin[1]]);
  typescriptConfig(writer, framework.compilerOptions);
  files.set(
    "index.html",
    `<!doctype html>
<html lang="en">
  <head>
    <meta charset="utf-8" />
    <link rel="icon" href="data:," />
  </head>
  <body>
    <div id="app"></div>
    <script type="module" src="/src/main.ts${extension === "tsx" ? "x" : ""}"></script>
  </body>
</html>
`,
  );
  framework.components(writer, components);
  for (const route of routes) {
    const file = `src/routes/r${route.index}.${extension}`;
    files.set(file, framework.route(writer, file, route));
  }
  files.set(
    `src/main.ts${extension === "tsx" ? "x" : ""}`,
    `${framework.mount[0]}

const routes = import.meta.glob("./routes/r*.${extension}");
const load = routes[\`./routes/\${location.pathname.slice(1) || "r0"}.${extension}\`] ?? routes["./routes/r0.${extension}"];
void load().then((module) => {
  const Route = (module as { default: any }).default;
  const target = document.getElementById("app")!;
  ${framework.mount[1]}
  document.documentElement.dataset.hydrated = "yes";
});
`,
  );
}

const vue: SinglePage = {
  plugin: ['import vue from "@vitejs/plugin-vue";', "vue()"],
  compilerOptions: { jsx: "preserve" },
  mount: ['import { createApp } from "vue";', "createApp(Route).mount(target);"],
  components(writer, components) {
    for (const { index, next, base } of components) {
      const file = `src/components/c${index}.vue`;
      writer.files.set(
        file,
        `<script lang="ts">
import { style } from "${writer.style(file)}";
import { ${base}, button, caption, primaryButton } from "${writer.ui(file)}";
${next === undefined ? "" : `import { c${next}Frame } from "./c${next}.vue";\n`}
export const c${index}Frame = style(${next === undefined ? base : `c${next}Frame`}, "p-${(index % 4) + 1}");
</script>

<script setup lang="ts">
${next === undefined ? "" : `import C${next} from "./c${next}.vue";\n\n`}const { active = false } = defineProps<{ active?: boolean }>();
const frame = style(c${index}Frame, "gap-2", {
  "hover:": "bg-gray-50",
});
${index === 0 ? 'const hmrLeaf = style("p-2 border-2");\n' : ""}</script>

<template>
  <section v-bind="style(frame)">
    <h3 v-bind="style('text-sm font-semibold', active && 'text-blue-700')">Component ${index}</h3>
    <p v-bind="style(caption, 'mt-${index % 6}')">Generated component ${index}</p>
    <button v-bind="style(active ? primaryButton : button)">Action</button>
${index === 0 ? '    <div id="hmr-leaf" v-bind="style(hmrLeaf)">leaf</div>\n' : ""}${next === undefined ? "" : `    <C${next} :active="!active" />\n`}  </section>
</template>
`,
      );
    }
  },
  route(writer, file, route) {
    return `<script setup lang="ts">
import { style } from "${writer.style(file)}";
import { heading, hmrShared, stack } from "${writer.ui(file)}";
${route.heads.map((head) => `import C${head} from "../components/c${head}.vue";`).join("\n")}

const layout = style(stack, "max-w-3xl mx-auto p-6 gap-4");
</script>

<template>
  <main v-bind="style(layout)">
    <h1 v-bind="style(heading)">Route ${route.index}</h1>
    <p id="hmr-theme" v-bind="style('text-xs text-brand')">theme</p>
    <div id="hmr-shared" v-bind="style(hmrShared)">shared</div>
    <nav v-bind="style('flex gap-2')">
      <a href="/r${route.link}">Next</a>
    </nav>
${route.heads.map((head) => `    <C${head} />`).join("\n")}
  </main>
</template>
`;
  },
};

/** Writes Svelte components. SvelteKit applications use the same components. */
function svelteComponents(writer: Writer, components: ComponentPlan[]) {
  for (const { index, next, base } of components) {
    const file = `src/components/c${index}.svelte`;
    writer.files.set(
      file,
      `<script module lang="ts">
  import { style } from "${writer.style(file)}";
  import { ${base}, button, caption, primaryButton } from "${writer.ui(file)}";
${next === undefined ? "" : `  import C${next}, { c${next}Frame } from "./c${next}.svelte";\n`}
  export const c${index}Frame = style(${next === undefined ? base : `c${next}Frame`}, "p-${(index % 4) + 1}");
</script>

<script lang="ts">
  let { active = false }: { active?: boolean } = $props();
  const frame = style(c${index}Frame, "gap-2", {
    "hover:": "bg-gray-50",
  });
${index === 0 ? '  const hmrLeaf = style("p-2 border-2");\n' : ""}</script>

<section {...style(frame)}>
  <h3 {...style("text-sm font-semibold", active && "text-blue-700")}>Component ${index}</h3>
  <p {...style(caption, "mt-${index % 6}")}>Generated component ${index}</p>
  <button {...style(active ? primaryButton : button)}>Action</button>
${index === 0 ? '  <div id="hmr-leaf" {...style(hmrLeaf)}>leaf</div>\n' : ""}${next === undefined ? "" : `  <C${next} active={!active} />\n`}</section>
`,
    );
  }
}

/** Returns a Svelte route. */
function svelteRoute(writer: Writer, file: string, route: RoutePlan) {
  return `<script lang="ts">
  import { style } from "${writer.style(file)}";
  import { heading, hmrShared, stack } from "${writer.ui(file)}";
${route.heads.map((head) => `  import C${head} from "${writer.request(file, `./src/components/c${head}.svelte`)}";`).join("\n")}

  const layout = style(stack, "max-w-3xl mx-auto p-6 gap-4");
</script>

<main {...style(layout)}>
  <h1 {...style(heading)}>Route ${route.index}</h1>
  <p id="hmr-theme" {...style("text-xs text-brand")}>theme</p>
  <div id="hmr-shared" {...style(hmrShared)}>shared</div>
  <nav {...style("flex gap-2")}>
    <a href="/r${route.link}">Next</a>
  </nav>
${route.heads.map((head) => `  <C${head} />`).join("\n")}
</main>
`;
}

const svelte: SinglePage = {
  plugin: ['import { svelte } from "@sveltejs/vite-plugin-svelte";', "svelte()"],
  compilerOptions: {},
  mount: ['import { mount } from "svelte";', "mount(Route, { target });"],
  components: svelteComponents,
  route: svelteRoute,
};

const solid: SinglePage = {
  plugin: ['import solid from "vite-plugin-solid";', "solid()"],
  compilerOptions: { jsx: "preserve", jsxImportSource: "solid-js" },
  mount: ['import { render } from "solid-js/web";', "render(() => <Route />, target);"],
  components: (writer, components) =>
    jsxComponents(writer, components, (props) => `${props}.active`),
  route: (writer, file, route) =>
    jsxRoute(writer, file, route, [], `<a href="/r${route.link}">Next</a>`),
};

function astro(writer: Writer, components: ComponentPlan[], routes: RoutePlan[]) {
  const { files } = writer;
  const stillwind = writer.variant === "stillwind";
  files.set(
    "astro.config.ts",
    `${stillwind ? 'import { stillwind } from "@stillwind/integration/astro";\n' : ""}import { defineConfig } from "astro/config";

export default defineConfig({
  integrations: [${stillwind ? "stillwind()" : ""}],
});
`,
  );
  typescriptConfig(writer, { jsx: "preserve" });
  for (const { index, next, base } of components) {
    const frame = `src/components/c${index}.frame.ts`;
    files.set(
      frame,
      `import { style } from "${writer.style(frame)}";
import { ${base} } from "${writer.ui(frame)}";
${next === undefined ? "" : `import { c${next}Frame } from "./c${next}.frame";\n`}
export const c${index}Frame = style(${next === undefined ? base : `c${next}Frame`}, "p-${(index % 4) + 1}");
`,
    );
    const file = `src/components/c${index}.astro`;
    files.set(
      file,
      `---
import { style } from "${writer.style(file)}";
import { button, caption, primaryButton } from "${writer.ui(file)}";
import { c${index}Frame } from "./c${index}.frame";
${next === undefined ? "" : `import C${next} from "./c${next}.astro";\n`}
const { active = false } = Astro.props as { active?: boolean };
const frame = style(c${index}Frame, "gap-2", {
  "hover:": "bg-gray-50",
});
${index === 0 ? 'const hmrLeaf = style("p-2 border-2");\n' : ""}---

<section {...style(frame)}>
  <h3 {...style("text-sm font-semibold", active && "text-blue-700")}>Component ${index}</h3>
  <p {...style(caption, "mt-${index % 6}")}>Generated component ${index}</p>
  <button {...style(active ? primaryButton : button)}>Action</button>
${index === 0 ? '  <div id="hmr-leaf" {...style(hmrLeaf)}>leaf</div>\n' : ""}${next === undefined ? "" : `  <C${next} active={!active} />\n`}</section>
`,
    );
  }
  for (const route of routes) {
    const file = `src/pages/${route.index === 0 ? "index" : `r${route.index}`}.astro`;
    files.set(
      file,
      `---
import { style } from "${writer.style(file)}";
import { heading, hmrShared, stack } from "${writer.ui(file)}";
${route.heads.map((head) => `import C${head} from "../components/c${head}.astro";`).join("\n")}

const layout = style(stack, "max-w-3xl mx-auto p-6 gap-4");
---

<html lang="en">
  <head>
    <link rel="icon" href="data:," />
  </head>
  <body>
    <main {...style(layout)}>
      <h1 {...style(heading)}>Route ${route.index}</h1>
      <p id="hmr-theme" {...style("text-xs text-brand")}>theme</p>
      <div id="hmr-shared" {...style(hmrShared)}>shared</div>
      <nav {...style("flex gap-2")}>
        <a href="/r${route.link}">Next</a>
      </nav>
${route.heads.map((head) => `      <C${head} />`).join("\n")}
    </main>
    <script is:inline>
      document.documentElement.dataset.hydrated = "yes";
    </script>
  </body>
</html>
`,
    );
  }
}

function next(writer: Writer, components: ComponentPlan[], routes: RoutePlan[]) {
  const { files } = writer;
  const stillwind = writer.variant === "stillwind";
  // Turbopack resolves only files below its root, and pnpm links packages to the workspace store.
  files.set(
    "next.config.js",
    `import { existsSync } from "node:fs";
import { dirname, join } from "node:path";
${stillwind ? 'import { withStillwind } from "@stillwind/integration/next";\n' : ""}
let workspace = process.cwd();
while (!existsSync(join(workspace, "pnpm-workspace.yaml")) && dirname(workspace) !== workspace) {
  workspace = dirname(workspace);
}

const config = {
  turbopack: {
    root: workspace,
  },
};

export default ${stillwind ? "withStillwind(config)" : "config"};
`,
  );
  typescriptConfig(writer, { jsx: "react-jsx", allowJs: true, isolatedModules: true });
  jsxComponents(writer, components, (props) => `(${props}.active ?? false)`);
  for (const route of routes) {
    const file = route.index === 0 ? "app/page.tsx" : `app/r${route.index}/page.tsx`;
    files.set(
      file,
      jsxRoute(
        writer,
        file,
        route,
        ['import Link from "next/link";'],
        `<Link href="/r${route.link}">Next</Link>`,
      ),
    );
  }
  files.set(
    "app/hydrated.tsx",
    `"use client";

import { useEffect } from "react";

export default function Hydrated() {
  useEffect(() => {
    document.documentElement.dataset.hydrated = "yes";
  }, []);
  return null;
}
`,
  );
  files.set(
    "app/layout.tsx",
    `import type { ReactNode } from "react";

import Hydrated from "./hydrated";

export default function Layout({ children }: { children: ReactNode }) {
  return (
    <html lang="en">
      <head>
        <link rel="icon" href="data:," />
      </head>
      <body>
        {children}
        <Hydrated />
      </body>
    </html>
  );
}
`,
  );
}

function sveltekit(writer: Writer, components: ComponentPlan[], routes: RoutePlan[]) {
  const { files } = writer;
  viteConfig(writer, ['import { sveltekit } from "@sveltejs/kit/vite";'], ["sveltekit()"]);
  // The Node adapter copies the package metadata into the server output.
  files.set("package.json", '{ "name": "benchmark-app", "private": true, "type": "module" }\n');
  files.set(
    "svelte.config.js",
    `import adapter from "@sveltejs/adapter-node";

export default {
  kit: {
    adapter: adapter(),
  },
};
`,
  );
  files.set(
    "tsconfig.json",
    JSON.stringify(
      {
        extends: "./.svelte-kit/tsconfig.json",
        compilerOptions: { strict: true, skipLibCheck: true },
      },
      null,
      2,
    ) + "\n",
  );
  files.set(
    "src/app.html",
    `<!doctype html>
<html lang="en">
  <head>
    <meta charset="utf-8" />
    <link rel="icon" href="data:," />
    %sveltekit.head%
  </head>
  <body>
    <div style="display: contents">%sveltekit.body%</div>
  </body>
</html>
`,
  );
  if (writer.variant === "stillwind") {
    files.set(
      "src/hooks.server.ts",
      'export { handle } from "@stillwind/integration/sveltekit/handle";\n',
    );
  }
  files.set(
    "src/routes/+layout.svelte",
    `<script lang="ts">
  import { onMount } from "svelte";

  let { children } = $props();
  onMount(() => {
    document.documentElement.dataset.hydrated = "yes";
  });
</script>

{@render children()}
`,
  );
  svelteComponents(writer, components);
  for (const route of routes) {
    const file =
      route.index === 0 ? "src/routes/+page.svelte" : `src/routes/r${route.index}/+page.svelte`;
    files.set(file, svelteRoute(writer, file, route));
  }
}
