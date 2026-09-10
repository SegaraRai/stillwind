///<reference types="svelte" />
;
import { style } from "stillwind/react";
function $$render() {


const items = [{tone:'red'}];
;
async () => {
  for(let item of __sveltets_2_ensureArray(items)){ { svelteHTML.createElement("div", {...style('template-scope', item.tone),}); }}};
return { props: {} as Record<string, never>, exports: {}, bindings: "", slots: {}, events: {} }}
const SvelteScopedExpression__SvelteComponent_ = __sveltets_2_isomorphic_component(__sveltets_2_with_any_event($$render()));
/*Ωignore_startΩ*/type SvelteScopedExpression__SvelteComponent_ = InstanceType<typeof SvelteScopedExpression__SvelteComponent_>;
/*Ωignore_endΩ*/export default SvelteScopedExpression__SvelteComponent_;