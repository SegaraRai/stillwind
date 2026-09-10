import 'svelte/internal/disclose-version';
import 'svelte/internal/flags/legacy';
import * as $ from 'svelte/internal/client';
import { style } from "stillwind/react";

var root = $.from_html(`<div></div>`);

export default function Svelte_duplicates($$anchor, $$props) {
	$.push($$props, false);

	const a = style('same');
	const b = style('same');

	$.init();

	var div = root();

	$.attribute_effect(div, ($0) => ({ ...$0 }), [() => style('same')]);
	$.append($$anchor, div);
	$.pop();
}