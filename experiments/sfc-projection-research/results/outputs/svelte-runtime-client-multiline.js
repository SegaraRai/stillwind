import 'svelte/internal/disclose-version';
import 'svelte/internal/flags/legacy';
import * as $ from 'svelte/internal/client';
import { style } from "stillwind/react";

var root = $.from_html(`<div></div>`);

export default function Svelte_multiline($$anchor, $$props) {
	$.push($$props, false);

	const props = style(
		/* 日本語 🧭 */
		'script-multi'
	);

	$.init();

	var div = root();

	$.attribute_effect(div, ($0) => ({ ...$0 }), [() => style('template-multi')]);
	$.append($$anchor, div);
	$.pop();
}