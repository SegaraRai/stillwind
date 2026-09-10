import { defineComponent as _defineComponent } from 'vue'
import { style } from "stillwind/react";

export default /*@__PURE__*/_defineComponent({
  __name: 'vue-scoped-expression',
  setup(__props, { expose: __expose }) {
  __expose();

const items = [{tone:'red'}];

const __returned__ = { items, get style() { return style } }
Object.defineProperty(__returned__, '__isScriptSetup', { enumerable: false, value: true })
return __returned__
}

})