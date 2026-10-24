import { Link } from "gatsby";
import React, { useState } from "react";
import { style } from "stillwind/react";

import { probeComposed, probeDiamond } from "../semantics-main";
import { shared } from "../shared-style";

export default function App() {
  const [count, setCount] = useState(0);
  const [phase, setPhase] = useState(0);
  const [tone, setTone] = useState("red");

  return (
    <>
      <main>
        <div data-testid="dynamic" {...style("text-[color:var(--tone)]", { "--tone": tone })}>
          Dynamic
        </div>
        <button data-testid="dynamic-toggle" onClick={() => setTone("blue")}>
          Tone
        </button>
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
        aria-busy={phase > 0 ? "true" : "false"}
        {...style("m-[40px]", phase > 0 && (phase > 1 ? "m-[16px]" : "m-[24px]"))}
      >
        Conditional
      </div>
      <div data-testid="main-conditional-active" {...style("m-[40px]", phase >= 0 && "m-[16px]")}>
        Active
      </div>
      <button data-testid="main-toggle" onClick={() => setPhase((phase + 1) % 3)}>
        Condition
      </button>
      <Link data-testid="next" to="/next/">
        Next
      </Link>
    </>
  );
}
