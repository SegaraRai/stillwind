import { useEffect, useState } from "preact/hooks";
import { style } from "stillwind/preact";

import { probeComposed, probeDiamond } from "./semantics-preact";

export default function Counter() {
  const [count, setCount] = useState(0);
  const [phase, setPhase] = useState(0);

  useEffect(() => {
    document.documentElement.dataset.preactReady = "yes";
  }, []);

  return (
    <>
      <button
        data-testid="preact-counter"
        {...style("text-probe")}
        onClick={() => setCount(count + 1)}
      >
        {count}
      </button>
      <div data-testid="preact-composed" {...style(probeComposed)}>
        Composition
      </div>
      <div data-testid="preact-diamond" {...style(probeDiamond)}>
        Diamond
      </div>
      <div
        data-testid="preact-conditional"
        {...style("m-[40px]", phase > 0 && (phase > 1 ? "m-[16px]" : "m-[24px]"), {
          "aria-busy": phase > 0 ? "true" : "false",
        })}
      >
        Conditional
      </div>
      <div data-testid="preact-conditional-active" {...style("m-[40px]", phase >= 0 && "m-[16px]")}>
        Active
      </div>
      <button data-testid="preact-toggle" onClick={() => setPhase((phase + 1) % 3)}>
        Condition
      </button>
    </>
  );
}
