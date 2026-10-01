import { render } from "solid-js/web";

import App from "./App";

const dispose = render(() => <App />, document.getElementById("app")!);

if (import.meta.webpackHot) {
  import.meta.webpackHot.accept();
  import.meta.webpackHot.dispose(dispose);
}
