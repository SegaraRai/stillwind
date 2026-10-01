import { createElement } from "react";
import { hydrateRoot } from "react-dom/client";

import { props } from "./a";

hydrateRoot(
  document.querySelector("#root"),
  createElement("div", { id: "sample", ...props, style: { width: 300, height: 300 } }, "sample"),
);

window.hydratedProps = props;
