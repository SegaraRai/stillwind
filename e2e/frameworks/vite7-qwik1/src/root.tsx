import { component$ } from "@builder.io/qwik";
import { QwikCityProvider, RouterOutlet } from "@builder.io/qwik-city";

import "./styles.css";

export default component$(() => (
  <QwikCityProvider>
    <head>
      <meta charset="utf-8" />
      <link rel="icon" href="data:," />
      <title>Stillwind Qwik</title>
      {import.meta.env.DEV && (
        <link rel="stylesheet" href="/__stillwind/style.css?environment=ssr" />
      )}
    </head>
    <body>
      <RouterOutlet />
    </body>
  </QwikCityProvider>
));
