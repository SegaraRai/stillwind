import { defineComponent as _defineComponent } from 'vue'
import { unref as _unref, normalizeProps as _normalizeProps, guardReactiveProps as _guardReactiveProps, openBlock as _openBlock, createElementBlock as _createElementBlock } from "vue"

import { style } from "stillwind/react";
import { css as sheet } from "stillwind/module-css";

export default /*@__PURE__*/_defineComponent({
  __name: 'vue-module-css',
  setup(__props) {

const props = style('sheet-style');

return (_ctx: any,_cache: any) => {
  return (_openBlock(), _createElementBlock("div", _normalizeProps(_guardReactiveProps(_unref(props))), null, 16 /* FULL_PROPS */))
}
}

})