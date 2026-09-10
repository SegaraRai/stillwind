import 'svelte/internal/disclose-version';
import 'svelte/internal/flags/legacy';
import * as $ from 'svelte/internal/client';
import { style } from "stillwind/react";

var root = $.from_html(`<div></div>`);

export default function Svelte_scoped_expression($$anchor, $$props) {
	$.push($$props, false);

	const items = [{ tone: 'red' }];

	$.init();

	var fragment = $.comment();
	var node = $.first_child(fragment);

	$.each(node, 1, () => items, $.index, ($$anchor, item) => {
		var div = root();

		$.attribute_effect(div, ($0) => ({ ...$0 }), [() => style('template-scope', $.get(item).tone)]);
		$.append($$anchor, div);
	});

	$.append($$anchor, fragment);
	$.pop();
}