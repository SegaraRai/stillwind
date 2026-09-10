import * as $ from 'svelte/internal/server';
import { style } from "stillwind/react";

export default function Svelte_unicode($$renderer, $$props) {
	$$renderer.component(($$renderer) => {
		// 日本語 🧭
		const label = "é🧭";

		const props = style('script-é🧭');

		$$renderer.push(`<div${$.attributes({ ...style('template-中🧭') })}></div>`);
	});
}