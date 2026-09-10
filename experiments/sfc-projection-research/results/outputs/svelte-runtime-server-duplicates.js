import * as $ from 'svelte/internal/server';
import { style } from "stillwind/react";

export default function Svelte_duplicates($$renderer, $$props) {
	$$renderer.component(($$renderer) => {
		const a = style('same');
		const b = style('same');

		$$renderer.push(`<div${$.attributes({ ...style('same') })}></div>`);
	});
}