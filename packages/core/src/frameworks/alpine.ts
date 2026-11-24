import { applyStyle, clearStyle } from "../dom/style.ts";
import {
  style as classObjectStyle,
  type StyleMacroWithoutForward,
  type StyleProps,
} from "../outputs/class-object.ts";

export type * from "../macro/index.ts";
export type { StyleResult as StyleResultOf } from "../macro/output.ts";
export type {
  StyleMacroWithoutForward as StyleMacro,
  StyleProps,
  StyleResult,
} from "../outputs/class-object.ts";

/** The `style` macro. Elements here receive no component props, so it has no `forward()`. */
export const style: StyleMacroWithoutForward = classObjectStyle;

type Alpine = {
  directive(
    name: string,
    handler: (
      element: HTMLElement,
      directive: {
        expression: string;
      },
      utilities: {
        evaluateLater(expression: string): (receive: (props: Readonly<StyleProps>) => void) => void;
        effect(callback: () => void): void;
        cleanup(callback: () => void): void;
      },
    ) => void,
  ): unknown;
  mutateDom<T>(callback: () => T): T;
};

/**
 * Registers `x-sw` for reactive compiled props. Install with `Alpine.plugin(stillwind)`
 * before `Alpine.start()`. The expression must return a Stillwind props object.
 * Updates replace the directive's contribution, and directive removal clears it.
 */
export function stillwind(alpine: Alpine): void {
  alpine.directive("sw", (element, { expression }, utilities) => {
    const evaluate = utilities.evaluateLater(expression);
    utilities.effect(() => evaluate((props) => alpine.mutateDom(() => applyStyle(element, props))));
    utilities.cleanup(() => alpine.mutateDom(() => clearStyle(element)));
  });
}
