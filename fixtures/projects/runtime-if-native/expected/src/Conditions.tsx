import { stillwindCssValue } from "stillwind/runtime/class-name-object";

export function Conditions({ enabled, second }: { enabled: boolean; second: boolean }) {
  return (
    <section
      {...{className:"sw_wmvay",style:{"--enabled":stillwindCssValue(enabled),"--second":stillwindCssValue(second)}}}
    >
      <div {...{className:"sw_b4p9x"}} />
      <div
        {...{className:"sw_93e4i",style:{"--mode":"compact"}}}
      />
    </section>
  );
}
