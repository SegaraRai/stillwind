import { style } from "stillwind/react";

export const menu = style.ref("menu");
export const trigger = style.ref("trigger");

export const triggerHovered = style.when(trigger("hover").in(menu));

export const wideMenu = style.when(menu.at("width >= 40rem"));

export const wideAndHovered = style.when(`${wideMenu}:${triggerHovered}`);

export const pressed = style.when("active:focus-visible");
