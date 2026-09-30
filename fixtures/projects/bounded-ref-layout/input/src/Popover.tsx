import { style } from "stillwind/react";

const menu = style.ref("menu");
const container = style.ref(menu, "container");
const trigger = style.ref(menu, "trigger");

export function Popover() {
  return (
    <section {...style(menu)}>
      <div
        {...style(container, style.anchorScope([trigger]), container.asContainer("inline-size"))}
      >
        <button {...style(trigger, trigger.asAnchor(), "relative inline-flex")}>Open</button>
        <div
          {...style(
            trigger.positionAnchor(),
            { [container.at("width >= 40rem")]: "grid-cols-2" },
            `${trigger`hover:`}:text-accent`,
            "absolute",
          )}
        />
      </div>
    </section>
  );
}
