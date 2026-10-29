/**
 * Sets whether the site header floats at the top of the viewport and whether a floating
 * header is shown. A hidden header is inert. The root element has `data-floating-header`
 * while a floating header is shown, so other fixed or sticky content can move below it.
 */
export function setHeaderState(header: HTMLElement, floating: boolean, visible: boolean): void {
  // Switching between document flow and the fixed position would otherwise animate the
  // hiding transition at the fixed position, so the header would briefly follow the page.
  const instant = floating !== header.hasAttribute("data-floating");
  if (instant) {
    header.style.transition = "none";
  }
  header.toggleAttribute("data-floating", floating);
  header.toggleAttribute("data-visible", visible);
  if (instant) {
    header.getBoundingClientRect();
    header.style.removeProperty("transition");
  }
  header.inert = !visible;
  header.ownerDocument.documentElement.toggleAttribute("data-floating-header", floating && visible);
}

/** Returns the header to document flow and removes the states that `setHeaderState` sets. */
export function clearHeaderState(header: HTMLElement): void {
  header.removeAttribute("data-floating");
  header.removeAttribute("data-visible");
  header.inert = false;
  header.ownerDocument.documentElement.removeAttribute("data-floating-header");
}
