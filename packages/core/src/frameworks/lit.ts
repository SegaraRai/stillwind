import { nothing } from "lit";
import { AsyncDirective } from "lit/async-directive.js";
import { directive, PartType, type ElementPart, type PartInfo } from "lit/directive.js";

import { applyStyle as apply, clearStyle, type StyleElement } from "../dom/style.ts";
import type { StyleProps } from "../outputs/class-object.ts";

export type * from "../macro/index.ts";
export { style } from "../outputs/class-object.ts";
export type { StyleMacro, StyleProps, StyleResult } from "../outputs/class-object.ts";

class StyleDirective extends AsyncDirective {
  #element?: StyleElement;
  #props?: Readonly<StyleProps>;

  constructor(part: PartInfo) {
    super(part);
    if (part.type !== PartType.ELEMENT) {
      throw new Error("Stillwind applyStyle must be used in a Lit element binding");
    }
  }

  override render(_props: Readonly<StyleProps>): typeof nothing {
    return nothing;
  }

  override update(part: ElementPart, [props]: [Readonly<StyleProps>]): typeof nothing {
    this.#element = part.element as StyleElement;
    this.#props = props;
    if (this.isConnected) {
      apply(this.#element, props);
    }
    return nothing;
  }

  protected override disconnected(): void {
    if (this.#element) {
      clearStyle(this.#element);
    }
  }

  protected override reconnected(): void {
    if (this.#element && this.#props) {
      apply(this.#element, this.#props);
    }
  }
}

/**
 * Applies compiled props in a browser Lit element binding: `<div ${applyStyle(props)}>`.
 * Updates replace its contribution and disconnection clears it. Reconnection reapplies
 * the latest props. Server rendering of this directive is unsupported.
 */
export const applyStyle = directive(StyleDirective);
