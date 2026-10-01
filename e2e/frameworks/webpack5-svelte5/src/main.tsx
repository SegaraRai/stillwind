import { mount, unmount } from "svelte";

import App from "./App.svelte";

const app = mount(App, { target: document.getElementById("app")! });

if (import.meta.webpackHot) {
  import.meta.webpackHot.accept();
  import.meta.webpackHot.dispose(() => {
    void unmount(app);
  });
}
