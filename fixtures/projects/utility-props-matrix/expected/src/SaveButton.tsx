import { stillwindClsx } from "stillwind/react/runtime";

const loading = Date.now() % 2 === 0;
const emphasized = location.hash === "#emphasized";

export const buttonProps = {className:stillwindClsx("_s_cf52bb6a",(loading) && ("_s_e2c8d13d"),(emphasized) ? ("_s_4d6c84ca") : ("_s_32623023"),"legacy-button",loading && "is-loading"),"aria-busy":loading,"data-emphasis":emphasized ? "high" : "normal"};

export function SaveButton() {
  return <button {...buttonProps}>Save</button>;
}
