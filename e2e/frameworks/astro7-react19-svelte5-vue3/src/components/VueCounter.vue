<script setup lang="ts">
import { style } from "stillwind/vue";
import { ref, onMounted } from "vue";

import { probeComposed, probeDiamond } from "./semantics-vue";

const count = ref(0);
const phase = ref(0);

onMounted(() => {
  document.documentElement.dataset.vueReady = "yes";
});
</script>

<template>
  <button data-testid="vue-counter" v-bind="style('text-probe')" @click="count++">
    {{ count }}
  </button>
  <div data-testid="vue-composed" v-bind="style(probeComposed)">Composition</div>
  <div data-testid="vue-diamond" v-bind="style(probeDiamond)">Diamond</div>
  <div
    data-testid="vue-conditional"
    :aria-busy="phase > 0 ? 'true' : 'false'"
    v-bind="style('m-[40px]', phase > 0 && (phase > 1 ? 'm-[16px]' : 'm-[24px]'))"
  >
    Conditional
  </div>
  <div data-testid="vue-conditional-active" v-bind="style('m-[40px]', phase >= 0 && 'm-[16px]')">
    Active
  </div>
  <button data-testid="vue-toggle" @click="phase = (phase + 1) % 3">Condition</button>
</template>
