<script setup lang="ts">
import { style } from "stillwind/vue";
import { ref } from "vue";

import { probeComposed, probeDiamond } from "./semantics-main";
import { shared } from "./shared-style";

const phase = ref(0);
const count = ref(0);
const scriptProps = style("p-[7px]");
</script>

<template>
  <main>
    <div data-testid="styled" v-bind="style(shared, 'text-probe p-[12px]')">Styled</div>
    <span data-testid="script-style" v-bind="scriptProps">Script style</span>
    <button data-testid="counter" @click="count++">{{ count }}</button>
  </main>
  <div data-testid="main-composed" v-bind="style(probeComposed)">Composition</div>
  <div data-testid="main-diamond" v-bind="style(probeDiamond)">Diamond</div>
  <div
    data-testid="main-conditional"
    v-bind="
      style('m-[40px]', phase > 0 && (phase > 1 ? 'm-[16px]' : 'm-[24px]'), {
        'aria-busy': phase > 0 ? 'true' : 'false',
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
