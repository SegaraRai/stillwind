import type { StyleProps } from "../outputs/class-object.ts";

export type StyleElement = Element & ElementCSSInlineStyle;

type Declaration = {
  value: string;
  priority: string;
};

type AppliedDeclaration = {
  previous: Declaration;
  applied: Declaration;
};

type AppliedStyle = {
  classes: Set<string>;
  declarations: Map<string, AppliedDeclaration>;
};

const appliedStyles = new WeakMap<Element, AppliedStyle>();

/**
 * Applies compiled props to an HTML, SVG, or other element with inline CSS support.
 *
 * Each call replaces the previous Stillwind contribution on this element. Classes
 * already present before Stillwind adds them are preserved. Inline CSS declarations
 * use CSS property names, including custom properties. Removing a declaration restores
 * its previous inline value and priority if its value has not been changed externally.
 * Updates copy the supplied values, so later mutations of `props` do not change ownership.
 *
 * @param element - Element whose class list and inline CSS declarations are updated.
 * @param props - Compiled class and optional style props. Empty props clear the contribution.
 */
export function applyStyle(element: StyleElement, props: Readonly<StyleProps>): void {
  const state = appliedStyles.get(element) ?? {
    classes: new Set<string>(),
    declarations: new Map<string, AppliedDeclaration>(),
  };
  const classes = new Set((props.class ?? "").split(/[\t\n\f\r ]+/).filter(Boolean));
  for (const name of state.classes) {
    if (!classes.has(name)) {
      element.classList.remove(name);
      state.classes.delete(name);
    }
  }
  for (const name of classes) {
    if (!element.classList.contains(name)) {
      element.classList.add(name);
      state.classes.add(name);
    }
  }

  const declarations = new Map(Object.entries(props.style ?? {}));
  for (const [name, declaration] of state.declarations) {
    if (!declarations.has(name)) {
      restoreDeclaration(element.style, name, declaration);
      state.declarations.delete(name);
    }
  }
  for (const [name, value] of declarations) {
    const current = readDeclaration(element.style, name);
    const retained = state.declarations.get(name);
    const previous =
      retained && sameDeclaration(current, retained.applied) ? retained.previous : current;
    element.style.setProperty(name, value);
    state.declarations.set(name, {
      previous,
      applied: readDeclaration(element.style, name),
    });
  }

  if (state.classes.size > 0 || state.declarations.size > 0) {
    appliedStyles.set(element, state);
  } else {
    appliedStyles.delete(element);
  }
}

/**
 * Removes the current Stillwind contribution from an element.
 *
 * Removes classes added by Stillwind and restores overwritten inline CSS declarations.
 * A declaration changed externally after the last application is preserved.
 *
 * @param element - Element previously passed to `applyStyle`. Other elements are unchanged.
 */
export function clearStyle(element: StyleElement): void {
  const state = appliedStyles.get(element);
  if (!state) {
    return;
  }
  for (const name of state.classes) {
    element.classList.remove(name);
  }
  for (const [name, declaration] of state.declarations) {
    restoreDeclaration(element.style, name, declaration);
  }
  appliedStyles.delete(element);
}

function readDeclaration(style: CSSStyleDeclaration, name: string): Declaration {
  return { value: style.getPropertyValue(name), priority: style.getPropertyPriority(name) };
}

function sameDeclaration(left: Declaration, right: Declaration): boolean {
  return left.value === right.value && left.priority === right.priority;
}

function restoreDeclaration(
  style: CSSStyleDeclaration,
  name: string,
  declaration: AppliedDeclaration,
): void {
  if (!sameDeclaration(readDeclaration(style, name), declaration.applied)) {
    return;
  }
  if (declaration.previous.value === "") {
    style.removeProperty(name);
  } else {
    style.setProperty(name, declaration.previous.value, declaration.previous.priority);
  }
}
