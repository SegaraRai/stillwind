

const outer = undefined;
const menu = undefined;
const trigger = undefined;

export function Scopes() {
  return (
    <section {...{className:"sw_outer_mm2en"}}>
      <div {...{className:"sw_lc4mp sw_menu_ja0oq"}} tabIndex={0}>
        <button {...{className:"sw_trigger_7z9al"}}>
          Trigger
          <span {...{className:"sw_3fwti"}}>Descendant</span>
        </button>
        <span {...{className:"sw_9v9ia"}}>Presence</span>
        <span {...{className:"sw_dqr19"}}>
          Stateful scope
        </span>
        <span {...{className:"sw_ux4n0"}}>Tagged scope</span>
        <span {...{className:"sw_anm5g"}}>Two scopes</span>
        <span {...{className:"sw_o646i"}}>
          Reversed scopes
        </span>
        <span
          {...{className:"sw_1ml1x"}}
        >
          Container and selector
        </span>
      </div>
    </section>
  );
}
