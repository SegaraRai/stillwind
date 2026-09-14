import { stillwindCssValue } from "stillwind/react/runtime";

export function Conditions({ enabled, second }: { enabled: boolean; second: boolean }) {
  return (
    <section
      {...{className:"_s_465c1769",style:{"--enabled":stillwindCssValue(enabled),"--second":stillwindCssValue(second)}}}
    >
      <div {...{className:"_s_f8c0af1e"}} />
      <div
        {...{className:"_s_5a7841d0",style:{"--mode":stillwindCssValue("compact")}}}
      />
    </section>
  );
}
