import { createElement } from "react";
import { hydrateRoot } from "react-dom/client";

import { App } from "./flow";

hydrateRoot(document.querySelector("#root"), createElement(App));
window.hydrated = true;
