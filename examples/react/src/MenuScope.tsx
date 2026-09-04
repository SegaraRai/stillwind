import type { PropsWithChildren } from "react";
import { style } from "stillwind/react";

export const menuTrigger = style.ref("menu-trigger");
export const alternateTrigger = style.ref("alternate-trigger");

export function MenuScope({
  children,
  alternate = false,
}: PropsWithChildren<{ alternate?: boolean }>) {
  const selected = alternate ? alternateTrigger : menuTrigger;
  return <section {...style(selected)}>{children}</section>;
}
