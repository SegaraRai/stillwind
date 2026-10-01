import { useEffect, useState } from "react";
import { Link, useLoaderData } from "react-router";
import { style } from "stillwind/react";

import { probeComposed, probeDiamond } from "../semantics-main";
import { serverProps } from "../server.server";
import "../server.server.js?stillwind.css";

export function loader() {
  return { props: serverProps };
}

export default function Home() {
  const data = useLoaderData();

  const [n, setN] = useState(0);
  const [phase, setPhase] = useState(0);

  useEffect(() => {
    document.documentElement.dataset.hydrated = "yes";
  }, []);

  return (
    <>
      <>
        <p id="server" {...data.props}>
          Server only style
        </p>
        <button id="client" {...style("brand")} onClick={() => setN(n + 1)}>
          {n}
        </button>
        <Link id="link" to="/lazy">
          Lazy
        </Link>
      </>
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
