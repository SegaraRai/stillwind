import { styleSheets } from "stillwind/css";
import { parts } from "stillwind/module-css";
import { applyStyle, clearStyle, style } from "stillwind/vanilla";

import { shared } from "./shared-style";

class ProbeCard extends HTMLElement {
  static observedAttributes = ["tone"];

  readonly #root = this.attachShadow({ mode: "open" });
  #count = 0;

  constructor() {
    super();

    this.#root.innerHTML = `
      <div data-testid="styled">Styled</div>
      <div data-testid="dynamic">Dynamic</div>
      <button data-testid="dynamic-toggle">Tone</button>
      <button data-testid="counter">0</button>
      <div data-testid="authored" class="authored">Authored</div>
    `;
    this.#root.querySelector<HTMLButtonElement>('[data-testid="dynamic-toggle"]')!.onclick = () => {
      this.tone = "blue";
    };
    const counter = this.#root.querySelector<HTMLButtonElement>('[data-testid="counter"]')!;
    counter.onclick = () => {
      counter.textContent = String(++this.#count);
    };
  }

  get tone(): string {
    return this.getAttribute("tone") ?? "red";
  }

  set tone(value: string) {
    this.setAttribute("tone", value);
  }

  connectedCallback(): void {
    this.#root.adoptedStyleSheets = styleSheets(parts, this.ownerDocument);
    applyStyle(
      this.#root.querySelector<HTMLElement>('[data-testid="styled"]')!,
      style(shared, "text-probe p-[12px]"),
    );
    this.#updateTone();
  }

  disconnectedCallback(): void {
    clearStyle(this.#root.querySelector<HTMLElement>('[data-testid="styled"]')!);
    clearStyle(this.#root.querySelector<HTMLElement>('[data-testid="dynamic"]')!);
  }

  attributeChangedCallback(): void {
    this.#updateTone();
  }

  #updateTone(): void {
    applyStyle(
      this.#root.querySelector<HTMLElement>('[data-testid="dynamic"]')!,
      style("text-[color:var(--tone)]", { "--tone": this.tone }),
    );
  }
}

customElements.define("probe-card", ProbeCard);
