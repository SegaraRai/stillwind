import { defineComponent as _defineComponent } from 'vue'
import { unref as _unref, normalizeProps as _normalizeProps, guardReactiveProps as _guardReactiveProps, openBlock as _openBlock, createElementBlock as _createElementBlock } from "vue"

import { style } from "stillwind/react";

export default /*@__PURE__*/_defineComponent({
  __name: 'vue-duplicates',
  setup(__props) {

const a = style('same');
const b = style('same');

return (_ctx: any,_cache: any) => {
  return (_openBlock(), _createElementBlock("div", _normalizeProps(_guardReactiveProps(_unref(style)('same'))), null, 16 /* FULL_PROPS */))
}
}

})