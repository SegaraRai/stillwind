import { css } from "stillwind/module-css";
import { style } from "stillwind/react";

export default function Explicit() {
  return (
    <>
      <style>{css}</style>
      <p id="explicit" {...style("p-[19px] picture")}>
        explicit
      </p>
    </>
  );
}
