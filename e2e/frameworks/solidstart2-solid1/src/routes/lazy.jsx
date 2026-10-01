import { A } from "@solidjs/router";
import { style } from "stillwind/solid";

export default function Lazy() {
  return (
    <>
      <p id="lazy" {...style("lazy")}>
        Lazy
      </p>
      <A id="back" href="/">
        Back
      </A>
    </>
  );
}
