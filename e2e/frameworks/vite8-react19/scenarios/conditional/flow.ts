import { createElement, useState } from "react";
import { style } from "stillwind/react";

export function App() {
  const [state, setState] = useState({
    enabled: true,
    nested: true,
  });
  if (typeof window !== "undefined") {
    window.setState = setState;
  }
  const props = style("m-10", state.enabled && (state.nested ? "m-4" : "m-6"), {
    "aria-busy": state.enabled,
  });
  if (typeof window !== "undefined") {
    window.classes = props.className;
  }
  return createElement(
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
  );
}
