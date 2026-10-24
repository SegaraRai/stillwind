import { LitElement, html, unsafeCSS } from "lit";
import { applyStyle, style } from "stillwind/lit";
import { css } from "stillwind/module-css";

import { shared } from "./shared-style";

class ProbeCard extends LitElement {
  static override styles = unsafeCSS(css);
  static override properties = {
    count: { state: true },
    tone: { type: String },
  };

  declare count: number;
  declare tone: string;

  constructor() {
    super();

    this.count = 0;
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
      <div data-testid="authored" class="authored">Authored</div>
    `;
  }
}

customElements.define("probe-card", ProbeCard);
