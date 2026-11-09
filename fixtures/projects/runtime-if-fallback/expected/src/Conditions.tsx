import { stillwindCssValue } from "stillwind/runtime";

export function Conditions({ enabled, second }: { enabled: boolean; second: boolean }) {
  return (
    <section
      {...{className:"sw_jf7wf",style:{"--enabled":stillwindCssValue(enabled),"--second":stillwindCssValue(second)}}}
    >
      <div {...{className:"sw_lr6al"}} />
    </section>
  );
}
