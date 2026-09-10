/// <reference types="F:/Projects/shorth/experiments/sfc-projection-research/node_modules/.pnpm/@vue+language-core@3.3.11/node_modules/@vue/language-core/types/template-helpers.d.ts" />
/// <reference types="F:/Projects/shorth/experiments/sfc-projection-research/node_modules/.pnpm/@vue+language-core@3.3.11/node_modules/@vue/language-core/types/props-fallback.d.ts" />

import { style } from "stillwind/react";

import { style as sx } from "stillwind/react";
export const shared=sx('normal');
export default {} as typeof __VLS_export;;
const __VLS_self = (await import('vue')).defineComponent({name:'Two'});

const __VLS_export = await (async () => {
const props=style('setup');
// @ts-ignore
declare const { defineProps, defineSlots, defineEmits, defineExpose, defineModel, defineOptions, withDefaults, }: typeof import('vue');
type __VLS_SetupExposed = import('vue').ShallowUnwrapRef<{
props: typeof props;
}>;
const __VLS_ctx = {
...{} as InstanceType<__VLS_PickNotAny<typeof __VLS_self, new () => {}>>,
...{} as __VLS_SetupExposed,
};
type __VLS_LocalComponents = __VLS_SetupExposed;
type __VLS_GlobalComponents = import('vue').GlobalComponents;
let __VLS_components!: __VLS_LocalComponents & __VLS_GlobalComponents;
let __VLS_intrinsics!: import('vue/jsx-runtime').JSX.IntrinsicElements;
type __VLS_LocalDirectives = __VLS_SetupExposed;
let __VLS_directives!: __VLS_LocalDirectives & import('vue').GlobalDirectives;
__VLS_asFunctionalElement1(__VLS_intrinsics.div)({
...(__VLS_ctx.props),
});
type __VLS_RootEl = 
| __VLS_Elements['div'];
// @ts-ignore
[props,];
return (await import('vue')).defineComponent({
});
})();
