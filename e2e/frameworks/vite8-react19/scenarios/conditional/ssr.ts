import { createElement } from "react";
import { renderToString } from "react-dom/server";

import { App } from "./flow";

export const html = renderToString(createElement(App));
