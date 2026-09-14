import { stillwindClsx } from "stillwind/react/runtime";

const loading = Date.now() % 2 === 0;
const emphasized = location.hash === "#emphasized";

export const buttonProps = {className:stillwindClsx("_s_4b7bffee",(loading) && ("_s_d7909e1f"),(emphasized) ? ("_s_a87e225d") : ("_s_7bafcdd5"),"legacy-button",loading && "is-loading"),"aria-busy":loading,"data-emphasis":emphasized ? "high" : "normal"};

export function SaveButton() {
  return <button {...buttonProps}>Save</button>;
}
