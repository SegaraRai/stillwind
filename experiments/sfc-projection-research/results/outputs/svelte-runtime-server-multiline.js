import * as $ from 'svelte/internal/server';
import { style } from "stillwind/react";

export default function Svelte_multiline($$renderer, $$props) {
	$$renderer.component(($$renderer) => {
		const props = style(
			/* 日本語 🧭 */
			'script-multi'
		);

		$$renderer.push(`<div${$.attributes({ ...style('template-multi') })}></div>`);
	});
}