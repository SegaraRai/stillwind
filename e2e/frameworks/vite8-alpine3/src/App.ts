import Alpine from "alpinejs";
import { stillwind, style } from "stillwind/alpine";

import { probeComposed, probeDiamond } from "./semantics-main";
import { shared } from "./shared-style";

import "./styles.css";

Alpine.plugin(stillwind);
Alpine.data("app", () => ({
  count: 0,
  phase: 0,
  tone: "red",
  get dynamic() {
    return style("text-[color:var(--tone)]", { "--tone": this.tone });
  },
  get props() {
    return style(shared, "text-probe p-[12px]");
  },
  get composed() {
    return style(probeComposed);
  },
  get diamond() {
    return style(probeDiamond);
  },
  get conditional() {
    return style("m-[40px]", this.phase > 0 && (this.phase > 1 ? "m-[16px]" : "m-[24px]"));
  },
  get active() {
    return style("m-[40px]", this.phase >= 0 && "m-[16px]");
  },
}));
Alpine.start();
