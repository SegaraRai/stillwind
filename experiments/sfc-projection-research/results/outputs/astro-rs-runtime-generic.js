import { render as $$render, createComponent as $$createComponent, maybeRenderHead as $$maybeRenderHead, spreadAttributes as $$spreadAttributes, createMetadata as $$createMetadata } from "astro/runtime/server/index.js";
import { style } from "stillwind/react";
import * as $$module1 from "stillwind/react";
export const $$metadata = $$createMetadata("astro-generic.astro", {
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
const $$AstroGeneric = $$createComponent(($$result, $$props, $$slots) => {
	const props = style("script-generic");
	return $$render`${$$maybeRenderHead($$result)}<div${$$spreadAttributes(style("template-generic"))}></div>`;
}, "astro-generic.astro", undefined);
export default $$AstroGeneric;
