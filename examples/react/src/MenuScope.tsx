import type { PropsWithChildren } from "react";
import { style } from "stillwind/react";

export const menuTrigger = style.ref("menu-trigger");

export function MenuScope({ children }: PropsWithChildren) {
  return <section {...style(menuTrigger)}>{children}</section>;
}
