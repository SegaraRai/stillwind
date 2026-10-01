import { createSignal, onMount } from "solid-js";
import { css } from "stillwind/module-css";
import { style } from "stillwind/solid";

import { probeComposed, probeDiamond } from "./semantics-solid";

export { css };

export default function Counter() {
  onMount(() => {
    document.documentElement.dataset.solidReady = "yes";
  });

  const [count, setCount] = createSignal(0);
  const [phase, setPhase] = createSignal(0);

  return (
    <>
      <button
        data-testid="solid-counter"
        {...style("text-probe")}
        onClick={() => setCount(count() + 1)}
      >
        {count()}
      </button>
      <div data-testid="solid-composed" {...style(probeComposed)}>
        Composition
      </div>
      <div data-testid="solid-diamond" {...style(probeDiamond)}>
        Diamond
      </div>
      <div
        data-testid="solid-conditional"
        {...style("m-[40px]", phase() > 0 && (phase() > 1 ? "m-[16px]" : "m-[24px]"), {
          "aria-busy": String(phase() > 0),
        })}
      >
        Conditional
      </div>
      <div
        data-testid="solid-conditional-active"
        {...style("m-[40px]", phase() >= 0 && "m-[16px]")}
      >
        Active
      </div>
      <button data-testid="solid-toggle" onClick={() => setPhase((phase() + 1) % 3)}>
        Condition
      </button>
    </>
  );
}
