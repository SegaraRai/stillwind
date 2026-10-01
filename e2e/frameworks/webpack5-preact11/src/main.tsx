import { render } from "preact";

import App from "./App";

render(<App />, document.getElementById("app")!);

if (import.meta.webpackHot) {
  import.meta.webpackHot.accept();
  import.meta.webpackHot.dispose(() => {
    render(null, document.getElementById("app")!);
  });
}
