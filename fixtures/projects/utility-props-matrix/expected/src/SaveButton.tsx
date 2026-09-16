import { stillwindClsx } from "stillwind/react/runtime";

const loading = Date.now() % 2 === 0;
const emphasized = location.hash === "#emphasized";

export const buttonProps = {className:stillwindClsx("sw_oqsqz",(loading) && ("sw_tk9jb"),(emphasized) ? ("sw_zw6ja") : ("sw_26ya8"),"legacy-button",loading && "is-loading"),"aria-busy":loading,"data-emphasis":emphasized ? "high" : "normal"};

export function SaveButton() {
  return <button {...buttonProps}>Save</button>;
}
