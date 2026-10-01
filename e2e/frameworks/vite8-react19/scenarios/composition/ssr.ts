import { createElement } from "react";
import { renderToString } from "react-dom/server";

import { props } from "./a";

export { props };
export const html = renderToString(
  createElement(
    "div",
    {
      id: "sample",
      ...props,
      style: {
        width: 300,
        height: 300,
      },
    },
    "sample",
  ),
);
