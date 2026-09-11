import { style } from "stillwind/react";

const menu = style.scope("menu");
const trigger = style.ref(menu, "trigger");

export function Popover() {
  return (
    <section {...style(menu)}>
      <div {...style(trigger.anchorScope, trigger.container("inline-size"))}>
        <button {...style(trigger, trigger.anchor, "relative inline-flex")}>Open</button>
        <div
          {...style(
            trigger.position,
            trigger.at("width >= 40rem", "grid-cols-2"),
            `${trigger`hover:`}:text-accent`,
            "absolute",
          )}
        />
      </div>
    </section>
  );
}
