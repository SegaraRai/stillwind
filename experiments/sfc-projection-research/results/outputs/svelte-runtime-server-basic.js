import * as $ from 'svelte/internal/server';
import { style } from "stillwind/react";

export default function Svelte_basic($$renderer, $$props) {
	$$renderer.component(($$renderer) => {
		const props = style('script-basic');

		$$renderer.push(`<div${$.attributes({ ...style('template-basic') })}></div>`);
	});
}