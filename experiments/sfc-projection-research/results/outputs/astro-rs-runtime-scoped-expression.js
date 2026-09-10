import { render as $$render, createComponent as $$createComponent, maybeRenderHead as $$maybeRenderHead, spreadAttributes as $$spreadAttributes, createMetadata as $$createMetadata } from "astro/runtime/server/index.js";
import { style } from "stillwind/react";
import * as $$module1 from "stillwind/react";
export const $$metadata = $$createMetadata("astro-scoped-expression.astro", {
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
const $$AstroScopedExpression = $$createComponent(($$result, $$props, $$slots) => {
	const items = [{ tone: "red" }];
	return $$render`${items.map((item) => $$render`${$$maybeRenderHead($$result)}<div${$$spreadAttributes(style("template-scope", item.tone))}></div>`)}`;
}, "astro-scoped-expression.astro", undefined);
export default $$AstroScopedExpression;
