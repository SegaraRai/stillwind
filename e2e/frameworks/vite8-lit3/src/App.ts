import { LitElement, html, unsafeCSS } from "lit";
import { applyStyle, style } from "stillwind/lit";
import { css } from "stillwind/module-css";

import { probeComposed, probeDiamond } from "./semantics-main";
import { shared } from "./shared-style";

class ProbeApp extends LitElement {
  declare count: number;
  declare phase: number;
  declare tone: string;

  static override styles = unsafeCSS(css);
  static override properties = {
    count: { state: true },
    phase: { state: true },
    tone: { state: true },
  };

  constructor() {
    super();

    this.count = 0;
    this.phase = 0;
    this.tone = "red";
  }

  override render() {
    return html`
      <div data-testid="styled" ${applyStyle(style(shared, "text-probe p-[12px]"))}>Styled</div>
      <div
        data-testid="dynamic"
        ${applyStyle(style("text-[color:var(--tone)]", { "--tone": this.tone }))}
      >
        Dynamic
      </div>
      <button
        data-testid="dynamic-toggle"
        @click=${() => {
          this.tone = "blue";
        }}
      >
        Tone
      </button>
      <button data-testid="counter" @click=${() => this.count++}>${this.count}</button>
      <div data-testid="main-composed" ${applyStyle(style(probeComposed))}>Composition</div>
      <div data-testid="main-diamond" ${applyStyle(style(probeDiamond))}>Diamond</div>
      <div
        data-testid="main-conditional"
        aria-busy=${this.phase > 0 ? "true" : "false"}
        ${applyStyle(style("m-[40px]", this.phase > 0 && (this.phase > 1 ? "m-[16px]" : "m-[24px]")))}
      >
        Conditional
      </div>
      <div
        data-testid="main-conditional-active"
        ${applyStyle(style("m-[40px]", this.phase >= 0 && "m-[16px]"))}
      >
        Active
      </div>
      <button
        data-testid="main-toggle"
        @click=${() => {
          this.phase = (this.phase + 1) % 3;
        }}
      >
        Condition
      </button>
    `;
  }
}

customElements.define("probe-app", ProbeApp);
