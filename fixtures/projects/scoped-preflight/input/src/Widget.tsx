import { style } from "stillwind/react";

export function Widget() {
  return (
    <section className="widget">
      <button {...style("rounded-md px-4 py-2")}>Continue</button>
    </section>
  );
}
