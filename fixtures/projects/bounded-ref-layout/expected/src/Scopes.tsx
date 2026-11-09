

const outer = undefined;
const menu = undefined;
const trigger = undefined;

export function Scopes() {
  return (
    <section {...{className:"sw_outer_i2rf6"}}>
      <div {...{className:"sw_6yn9k sw_menu_hm9ig"}} tabIndex={0}>
        <button {...{className:"sw_trigger_27eps"}}>
          Trigger
          <span {...{className:"sw_aum6m"}}>Descendant</span>
        </button>
        <span {...{className:"sw_e2esu"}}>Presence</span>
        <span {...{className:"sw_joqen"}}>
          Stateful scope
        </span>
        <span {...{className:"sw_lissf"}}>Tagged scope</span>
        <span {...{className:"sw_qe0ad"}}>Two scopes</span>
        <span {...{className:"sw_1jw71"}}>
          Reversed scopes
        </span>
        <span
          {...{className:"sw_sg0kf"}}
        >
          Container and selector
        </span>
      </div>
    </section>
  );
}
