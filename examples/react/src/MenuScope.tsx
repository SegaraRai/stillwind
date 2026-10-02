import type { PropsWithChildren } from "react";
import { style } from "stillwind/react";

export const menuTrigger = style.ref("menu-trigger");
export const alternateTrigger = style.ref("alternate-trigger");
export const menuScope = style.ref("menu");

export function MenuScope({
  children,
  alternate = false,
}: PropsWithChildren<{ alternate?: boolean }>) {
  const selected = alternate ? alternateTrigger : menuTrigger;
  return (
    <section {...style(menuScope)}>
      <button {...style(selected)}>Hover the menu trigger</button>
      {children}
    </section>
  );
}
