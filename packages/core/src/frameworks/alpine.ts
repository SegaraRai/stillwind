import { applyStyle, clearStyle } from "../dom/style.ts";
import type { StyleProps } from "../outputs/class-object.ts";

export type * from "../macro/index.ts";
export { style } from "../outputs/class-object.ts";
export type { StyleMacro, StyleProps, StyleResult } from "../outputs/class-object.ts";

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
