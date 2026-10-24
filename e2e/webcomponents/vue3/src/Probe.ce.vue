<script lang="ts">
import { css } from "stillwind/module-css";

export default {
  styles: [css],
};
</script>

<script setup lang="ts">
import { style } from "stillwind/vue";
import { ref, useHost } from "vue";

import { shared } from "./shared-style";

const props = withDefaults(defineProps<{ tone?: string }>(), { tone: "red" });
const count = ref(0);
const host = useHost();
</script>

<template>
  <div data-testid="styled" v-bind="style(shared, 'text-probe p-[12px]')">Styled</div>
  <div data-testid="dynamic" v-bind="style('text-[color:var(--tone)]', { '--tone': props.tone })">
    Dynamic
  </div>
  <button data-testid="dynamic-toggle" @click="host?.setAttribute('tone', 'blue')">Tone</button>
  <button data-testid="counter" @click="count++">{{ count }}</button>
  <div data-testid="authored" class="authored">Authored</div>
</template>
