import { render as $$render, createComponent as $$createComponent, maybeRenderHead as $$maybeRenderHead, spreadAttributes as $$spreadAttributes, createMetadata as $$createMetadata } from "astro/runtime/server/index.js";
import { style } from "stillwind/react";
import * as $$module1 from "stillwind/react";
export const $$metadata = $$createMetadata("astro-multiline.astro", {
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
const $$AstroMultiline = $$createComponent(($$result, $$props, $$slots) => {
	const props = style(
		/* 日本語 🧭 */
		"script-multi"
	);
	return $$render`${$$maybeRenderHead($$result)}<div${$$spreadAttributes(style("template-multi"))}></div>`;
}, "astro-multiline.astro", undefined);
export default $$AstroMultiline;
