import * as $ from 'svelte/internal/server';
import { style } from "stillwind/react";

export default function Svelte_scoped_expression($$renderer, $$props) {
	$$renderer.component(($$renderer) => {
		const items = [{ tone: 'red' }];

		$$renderer.push(`<!--[-->`);

		const each_array = $.ensure_array_like(items);

		for (let $$index = 0, $$length = each_array.length; $$index < $$length; $$index++) {
			let item = each_array[$$index];

			$$renderer.push(`<div${$.attributes({ ...style('template-scope', item.tone) })}></div>`);
		}

		$$renderer.push(`<!--]-->`);
	});
}