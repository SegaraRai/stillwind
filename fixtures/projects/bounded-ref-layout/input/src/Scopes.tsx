import { style } from "stillwind/react";

const outer = style.ref("outer");
const menu = style.ref("menu");
const trigger = style.ref("trigger");

export function Scopes() {
  return (
    <section {...style(outer)}>
      <div {...style(menu, menu.asContainer("inline-size"))} tabIndex={0}>
        <button {...style(trigger)}>
          Trigger
          <span {...style({ [trigger("hover")]: "text-accent" })}>Descendant</span>
        </button>
        <span {...style({ [trigger().in(menu)]: "text-accent" })}>Presence</span>
        <span {...style({ [trigger("hover").in(menu("focus"))]: "text-accent" })}>
          Stateful scope
        </span>
        <span {...style({ [trigger`hover:`.in(menu`focus:`)]: "text-accent" })}>Tagged scope</span>
        <span {...style({ [trigger("hover").in(menu).in(outer)]: "text-accent" })}>Two scopes</span>
        <span {...style({ [trigger("hover").in(outer).in(menu)]: "text-accent" })}>
          Reversed scopes
        </span>
        <span
          {...style({
            [menu.at("width >= 40rem")]: {
              [trigger("hover").in(menu)]: "text-accent",
            },
          })}
        >
          Container and selector
        </span>
      </div>
    </section>
  );
}
