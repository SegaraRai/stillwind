/// <reference types="vite/client" />

import { resolve as resolvePath } from "$app/paths";
import type { Handle } from "@sveltejs/kit";

/**
 * Supplies development SSR CSS through SvelteKit's page transformation hook.
 * Use this handle with the Vite adapter; production uses Vite's emitted stylesheets.
 * SvelteKit's sequence helper can compose it with application handles.
 */
export const handle: Handle = ({ event, resolve }) =>
  resolve(event, {
    transformPageChunk: ({ html }) =>
      import.meta.env.DEV
        ? html.replace(
            "</head>",
            `<link rel="stylesheet" href="${resolvePath("/__stillwind/style.css?environment=ssr", {})}"></head>`,
          )
        : html,
  });
