

const outer = undefined;
const menu = undefined;
const trigger = undefined;

export function Scopes() {
  return (
    <section {...{className:"sw_outer_i2rf6"}}>
      <div {...{className:"sw_spo42 sw_menu_hm9ig"}} tabIndex={0}>
        <button {...{className:"sw_trigger_27eps"}}>
          Trigger
          <span {...{className:"sw_3g5fx"}}>Descendant</span>
        </button>
        <span {...{className:"sw_z7l65"}}>Presence</span>
        <span {...{className:"sw_73vwv"}}>
          Stateful scope
        </span>
        <span {...{className:"sw_a69g7"}}>Tagged scope</span>
        <span {...{className:"sw_18ny0"}}>Two scopes</span>
        <span {...{className:"sw_i1xzs"}}>
          Reversed scopes
        </span>
        <span
          {...{className:"sw_ajbkg"}}
        >
          Container and selector
        </span>
      </div>
    </section>
  );
}
