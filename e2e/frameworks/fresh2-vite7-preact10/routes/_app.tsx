import type { PageProps } from "fresh";

export default function App({ Component }: PageProps) {
  return (
    <html lang="en">
      <head>
        <meta charset="utf-8" />
        <link rel="icon" href="data:," />
        <title>Stillwind Fresh</title>
        {import.meta.env.DEV && (
          <link rel="stylesheet" href="/__stillwind/style.css?environment=ssr" />
        )}
      </head>
      <body>
        <Component />
      </body>
    </html>
  );
}
