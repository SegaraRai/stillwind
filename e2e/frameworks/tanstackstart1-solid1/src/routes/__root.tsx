import { createRootRoute, Outlet, HeadContent, Scripts } from "@tanstack/solid-router";
import { HydrationScript } from "solid-js/web";

export const Route = createRootRoute({
  component: () => (
    <html>
      <head>
        <link rel="icon" href="data:," />
        <HydrationScript />
        <HeadContent />
        {import.meta.env.DEV && (
          <link
            rel="stylesheet"
            href="/__stillwind/style.css?environment=ssr&module=src/routes/index.tsx"
          />
        )}
      </head>
      <body>
        <Outlet />
        <Scripts />
      </body>
    </html>
  ),
});
