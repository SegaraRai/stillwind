import { defineComponent as _defineComponent } from 'vue'
import { style } from "stillwind/react";

export default /*@__PURE__*/_defineComponent({
  __name: 'vue-duplicates',
  setup(__props, { expose: __expose }) {
  __expose();

const a = style('same');
const b = style('same');

const __returned__ = { a, b, get style() { return style } }
Object.defineProperty(__returned__, '__isScriptSetup', { enumerable: false, value: true })
return __returned__
}

})