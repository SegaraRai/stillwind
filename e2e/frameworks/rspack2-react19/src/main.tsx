import { createRoot } from "react-dom/client";

import App from "./App";

const root = createRoot(document.getElementById("app")!);
root.render(<App />);

if (import.meta.webpackHot) {
  import.meta.webpackHot.accept();
  import.meta.webpackHot.dispose(() => {
    root.unmount();
  });
}
