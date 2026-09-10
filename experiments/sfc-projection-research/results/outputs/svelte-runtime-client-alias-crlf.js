import 'svelte/internal/disclose-version';
import 'svelte/internal/flags/legacy';
import * as $ from 'svelte/internal/client';
import { style as sx } from "stillwind/react";

var root = $.from_html(`<div></div>`);

export default function Svelte_alias_crlf($$anchor, $$props) {
	$.push($$props, false);

	/* alias 🧭 */
	const props = sx('script-alias');

	$.init();

	var div = root();

	$.attribute_effect(div, ($0) => ({ ...$0 }), [() => sx('template-alias')]);
	$.append($$anchor, div);
	$.pop();
}