import { style } from "stillwind/react";

export function Conditions({ enabled, second }: { enabled: boolean; second: boolean }) {
  return (
    <section
      {...style(
        "grid if(--enabled):flex w-[--if(--enabled,120px,60px)] h-[--if(--enabled,--if(--second,30px,20px),10px)] opacity-100 if(--enabled):if(--second):opacity-50!",
        { "--enabled": enabled, "--second": second },
      )}
    >
      <div {...style("w-[--if(!--enabled,40px,80px)]")} />
    </section>
  );
}
