<script setup lang="ts">
import { style } from "stillwind/react";
const items = [{tone:'red'}];
</script>
<template><div v-for="item in items" v-bind="style('template-scope', item.tone)"/></template>