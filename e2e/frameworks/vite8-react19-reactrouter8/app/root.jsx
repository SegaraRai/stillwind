import { Links, Meta, Outlet, Scripts, ScrollRestoration } from "react-router";

export default function Root() {
  return (
    <html>
      <head>
        <link rel="icon" href="data:," />
        <Meta />
        <Links />
        {import.meta.env.DEV && (
          <link
            rel="stylesheet"
            href="/__stillwind/style.css?environment=ssr&module=app/routes/home.jsx&module=app/server.server.js"
          />
        )}
      </head>
      <body>
        <Outlet />
        <ScrollRestoration />
        <Scripts />
      </body>
    </html>
  );
}
