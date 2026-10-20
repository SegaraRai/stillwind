import { style } from "stillwind/vue";
import { defineComponent, ref } from "vue";

import { probeComposed, probeDiamond } from "./semantics-main";
import { shared } from "./shared-style";

export default defineComponent({
  setup() {
    const count = ref(0);
    const phase = ref(0);

    return () => (
      <>
        <main>
          <div data-testid="styled" {...style(shared, "text-probe p-[12px]")}>
            Styled
          </div>
          <button data-testid="counter" onClick={() => count.value++}>
            {count.value}
          </button>
        </main>
        <div data-testid="main-composed" {...style(probeComposed)}>
          Composition
        </div>
        <div data-testid="main-diamond" {...style(probeDiamond)}>
          Diamond
        </div>
        <div
          data-testid="main-conditional"
          aria-busy={phase.value > 0 ? "true" : "false"}
          {...style("m-[40px]", phase.value > 0 && (phase.value > 1 ? "m-[16px]" : "m-[24px]"))}
        >
          Conditional
        </div>
        <div
          data-testid="main-conditional-active"
          {...style("m-[40px]", phase.value >= 0 && "m-[16px]")}
        >
          Active
        </div>
        <button data-testid="main-toggle" onClick={() => (phase.value = (phase.value + 1) % 3)}>
          Condition
        </button>
      </>
    );
  },
});
