import { createFileRoute } from "@tanstack/react-router";
import { useState, useEffect } from "react";
import { style } from "stillwind/react";

import { probeComposed, probeDiamond } from "../semantics-main";
import { shared } from "../shared-style";

function Home() {
  const [count, setCount] = useState(0);
  const [phase, setPhase] = useState(0);

  useEffect(() => {
    document.documentElement.dataset.hydrated = "yes";
  }, []);

  return (
    <>
      <main>
        <div data-testid="styled" {...style(shared, "text-probe p-[12px]")}>
          Styled
        </div>
        <button data-testid="counter" onClick={() => setCount(count + 1)}>
          {count}
        </button>
      </main>
      <div data-testid="main-composed" {...style(probeComposed)}>
        Composition
      </div>
      <div data-testid="main-diamond" {...style(probeDiamond)}>
        Diamond
      </div>
      <div
        data-testid="main-conditional"
        {...style("m-[40px]", phase > 0 && (phase > 1 ? "m-[16px]" : "m-[24px]"), {
          "aria-busy": String(phase > 0),
        })}
      >
        Conditional
      </div>
      <div data-testid="main-conditional-active" {...style("m-[40px]", phase >= 0 && "m-[16px]")}>
        Active
      </div>
      <button data-testid="main-toggle" onClick={() => setPhase((phase + 1) % 3)}>
        Condition
      </button>
    </>
  );
}

export const Route = createFileRoute("/")({ component: Home });
