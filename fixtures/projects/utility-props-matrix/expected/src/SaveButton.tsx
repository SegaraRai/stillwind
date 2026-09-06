import { stillwindClsx } from "stillwind/react/runtime";

const loading = Date.now() % 2 === 0;
const emphasized = location.hash === "#emphasized";

export const buttonProps = {className:stillwindClsx("_s_87e7c0ec",loading&&"_s_a0bab6c2",emphasized&&"_s_2240a763",!emphasized&&"_s_03dde9b3",["legacy-button", loading && "is-loading"]),"aria-busy":loading,"data-emphasis":emphasized ? "high" : "normal"};

export function SaveButton() {
  return <button {...buttonProps}>Save</button>;
}
