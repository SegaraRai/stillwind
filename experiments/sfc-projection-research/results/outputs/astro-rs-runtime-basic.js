import { render as $$render, createComponent as $$createComponent, maybeRenderHead as $$maybeRenderHead, spreadAttributes as $$spreadAttributes, createMetadata as $$createMetadata } from "astro/runtime/server/index.js";
import { style } from "stillwind/react";
import * as $$module1 from "stillwind/react";
export const $$metadata = $$createMetadata("astro-basic.astro", {
	modules: [{
		module: $$module1,
		specifier: "stillwind/react",
		assert: {}
	}],
	hydratedComponents: [],
	clientOnlyComponents: [],
	hydrationDirectives: new Set([]),
	hoisted: []
});
const $$AstroBasic = $$createComponent(($$result, $$props, $$slots) => {
	const props = style("script-basic");
	return $$render`${$$maybeRenderHead($$result)}<div${$$spreadAttributes(style("template-basic"))}></div>`;
}, "astro-basic.astro", undefined);
export default $$AstroBasic;
