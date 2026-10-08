import { A } from "@solidjs/router";
import { createSignal, onMount } from "solid-js";
import { style } from "stillwind/solid";

import { probeComposed, probeDiamond } from "../semantics-main";

export default function Home() {
  const [n, setN] = createSignal(-1);
  const [phase, setPhase] = createSignal(0);

  onMount(() => {
    document.documentElement.dataset.hydrated = "yes";
  });

  return (
    <>
      <>
        <p id="static" {...style("brand")}>
          SSR
        </p>
        <button id="client" {...style(n() > 0 ? "active" : "brand")} onClick={() => setN(n() + 2)}>
          {n()}
        </button>
        <A id="link" href="/lazy">
          Lazy
        </A>
      </>
      <div data-testid="main-composed" {...style(probeComposed)}>
        Composition
      </div>
      <div data-testid="main-diamond" {...style(probeDiamond)}>
        Diamond
      </div>
      <div
        data-testid="main-conditional"
        {...style("m-[40px]", phase() > 0 && (phase() > 1 ? "m-[16px]" : "m-[24px]"), {
          "aria-busy": phase() > 0 ? "true" : "false",
        })}
      >
        Conditional
      </div>
      <div data-testid="main-conditional-active" {...style("m-[40px]", phase() >= 0 && "m-[16px]")}>
        Active
      </div>
      <button data-testid="main-toggle" onClick={() => setPhase((phase() + 1) % 3)}>
        Condition
      </button>
    </>
  );
}
