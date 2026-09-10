import { defineComponent as _defineComponent } from 'vue'
import { renderList as _renderList, Fragment as _Fragment, openBlock as _openBlock, createElementBlock as _createElementBlock, unref as _unref, mergeProps as _mergeProps, createElementVNode as _createElementVNode } from "vue"

import { style } from "stillwind/react";

export default /*@__PURE__*/_defineComponent({
  __name: 'vue-scoped-expression',
  setup(__props) {

const items = [{tone:'red'}];

return (_ctx: any,_cache: any) => {
  return (_openBlock(), _createElementBlock(_Fragment, null, _renderList(items, (item) => {
    return _createElementVNode("div", _mergeProps({ ref_for: true }, _unref(style)('template-scope', item.tone)), null, 16 /* FULL_PROPS */)
  }), 64 /* STABLE_FRAGMENT */))
}
}

})