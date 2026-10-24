import { customElement } from "solid-element";
import { createSignal } from "solid-js";
import { styleSheets } from "stillwind/css";
import { parts } from "stillwind/module-css";
import { style } from "stillwind/solid";

import { shared } from "./shared-style";

customElement("probe-card", { tone: "red" }, (props, { element }) => {
  const [count, setCount] = createSignal(0);

  const root = element.renderRoot as ShadowRoot;
  root.adoptedStyleSheets = styleSheets(parts, element.ownerDocument);

  return (
    <>
      <div data-testid="styled" {...style(shared, "text-probe p-[12px]")}>
        Styled
      </div>
      <div data-testid="dynamic" {...style("text-[color:var(--tone)]", { "--tone": props.tone })}>
        Dynamic
      </div>
      <button data-testid="dynamic-toggle" onClick={() => element.setAttribute("tone", "blue")}>
        Tone
      </button>
      <button data-testid="counter" onClick={() => setCount(count() + 1)}>
        {count()}
      </button>
      <div data-testid="authored" class="authored">
        Authored
      </div>
    </>
  );
});
