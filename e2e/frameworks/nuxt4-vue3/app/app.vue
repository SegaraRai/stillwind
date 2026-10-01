<script setup lang="ts">
import { style } from "stillwind/vue";
import { ref, onMounted } from "vue";

import { shared } from "../src/shared-style";
import { probeComposed, probeDiamond } from "./semantics-main";

const count = ref(0);
const phase = ref(0);
const ssrHead = import.meta.env.DEV
  ? useHead({ link: [{ key: "stillwind-ssr", rel: "stylesheet", href: "/e2e-ssr.css" }] })
  : undefined;
onMounted(() => {
  ssrHead?.dispose();
  document.documentElement.dataset.hydrated = "yes";
});
</script>

<template>
  <main>
    <div data-testid="styled" v-bind="style(shared, 'text-probe p-[12px]')">Styled</div>
    <button data-testid="counter" @click="count++">{{ count }}</button>
  </main>
  <div data-testid="main-composed" v-bind="style(probeComposed)">Composition</div>
  <div data-testid="main-diamond" v-bind="style(probeDiamond)">Diamond</div>
  <div
    data-testid="main-conditional"
    v-bind="
      style('m-[40px]', phase > 0 && (phase > 1 ? 'm-[16px]' : 'm-[24px]'), {
        'aria-busy': String(phase > 0),
      })
    "
  >
    Conditional
  </div>
  <div data-testid="main-conditional-active" v-bind="style('m-[40px]', phase >= 0 && 'm-[16px]')">
    Active
  </div>
  <button data-testid="main-toggle" @click="phase = (phase + 1) % 3">Condition</button>
</template>
