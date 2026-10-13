/// <reference types="vite/client" />

import { resolve as resolvePath } from "$app/paths";
import type { sequence } from "@sveltejs/kit/hooks";

/**
 * Supplies development SSR CSS through SvelteKit's page transformation hook.
 * Use this handle with the Vite adapter; production uses Vite's emitted stylesheets.
 * SvelteKit's sequence helper can compose it with application handles.
 *
 * @param options - SvelteKit request event and the resolver that renders its response.
 * @returns The resolver's response. Development HTML chunks receive a stylesheet link before
 * their first closing head tag, and production chunks retain their original text.
 * @throws If request resolution or page transformation fails.
 */
export const handle: ReturnType<typeof sequence> = ({ event, resolve }) =>
  resolve(event, {
    transformPageChunk: ({ html }) =>
      import.meta.env.DEV
        ? html.replace(
            "</head>",
            `<link rel="stylesheet" href="${resolvePath("/__stillwind/style.css?environment=ssr", {})}"></head>`,
          )
        : html,
  });
