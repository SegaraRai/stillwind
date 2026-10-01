import { createApp } from "vue";

import App from "./App.vue";

const app = createApp(App);
app.mount("#app");

if (import.meta.webpackHot) {
  import.meta.webpackHot.accept();
  import.meta.webpackHot.dispose(() => {
    app.unmount();
  });
}
