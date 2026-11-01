import { parse, render } from "sugar-high/core";
import * as css from "sugar-high/lang/css";

/**
 * Returns escaped HTML for `code` with each CSS token in a span. The spans take their colors
 * from `--sh-*` custom properties, which `candidate-result-code` defines.
 */
export function highlightCss(code: string): string {
  return render(parse(code, css));
}
