import { stillwindClsx } from "stillwind/runtime";

const loading = Date.now() % 2 === 0;
const emphasized = location.hash === "#emphasized";

export const buttonProps = {className:stillwindClsx("sw_801ut",(loading) && ("sw_47yet"),(emphasized) ? ("sw_hra5a") : ("sw_lwn5s"),"legacy-button",loading && "is-loading")};

export function SaveButton() {
  return (
    <button {...buttonProps} aria-busy={loading} data-emphasis={emphasized ? "high" : "normal"}>
      Save
    </button>
  );
}
