import { createHandler, StartServer } from "@solidjs/start/server";

function Document(props) {
  return (
    <html>
      <head>
        <link rel="icon" href="data:," />
        {props.assets}
        {import.meta.env.DEV && (
          <link
            rel="stylesheet"
            href="/__stillwind/style.css?environment=ssr&module=src/routes/index.jsx"
          />
        )}
      </head>
      <body>
        <div id="app">{props.children}</div>
        {props.scripts}
      </body>
    </html>
  );
}

export default createHandler(() => <StartServer document={Document} />);
