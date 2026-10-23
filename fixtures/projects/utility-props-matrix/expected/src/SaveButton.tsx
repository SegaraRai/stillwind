import { stillwindClsx } from "stillwind/runtime/class-name-object";

const loading = Date.now() % 2 === 0;
const emphasized = location.hash === "#emphasized";

export const buttonProps = {className:stillwindClsx("sw_hpdco",(loading) && ("sw_hmw23"),(emphasized) ? ("sw_96vzq") : ("sw_ke6ey"),"legacy-button",loading && "is-loading")};

export function SaveButton() {
  return (
    <button {...buttonProps} aria-busy={loading} data-emphasis={emphasized ? "high" : "normal"}>
      Save
    </button>
  );
}
