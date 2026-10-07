import { useEffect, useState } from "react";
import { style } from "stillwind/react";

import { probeComposed, probeDiamond } from "./semantics-react";

export default function ReactCounter() {
  useEffect(() => {
    document.documentElement.dataset.reactReady = "yes";
  }, []);
  const [count, setCount] = useState(0);
  const [phase, setPhase] = useState(0);

  return (
    <>
      <button
        data-testid="react-counter"
        {...style("text-probe")}
        onClick={() => setCount(count + 1)}
      >
        {count}
      </button>
      <div data-testid="react-composed" {...style(probeComposed)}>
        Composition
      </div>
      <div data-testid="react-diamond" {...style(probeDiamond)}>
        Diamond
      </div>
      <div
        data-testid="react-conditional"
        {...style("m-[40px]", phase > 0 && (phase > 1 ? "m-[16px]" : "m-[24px]"), {
          "aria-busy": phase > 0 ? "true" : "false",
        })}
      >
        Conditional
      </div>
      <div data-testid="react-conditional-active" {...style("m-[40px]", phase >= 0 && "m-[16px]")}>
        Active
      </div>
      <button data-testid="react-toggle" onClick={() => setPhase((phase + 1) % 3)}>
        Condition
      </button>
    </>
  );
}
