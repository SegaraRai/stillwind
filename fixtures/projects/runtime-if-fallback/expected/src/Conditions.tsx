import { stillwindCssValue } from "stillwind/react/runtime";

export function Conditions({ enabled, second }: { enabled: boolean; second: boolean }) {
  return (
    <section
      {...{className:"sw_j49u5",style:{"--enabled":stillwindCssValue(enabled),"--second":stillwindCssValue(second)}}}
    >
      <div {...{className:"sw_mdfmd"}} />
    </section>
  );
}
