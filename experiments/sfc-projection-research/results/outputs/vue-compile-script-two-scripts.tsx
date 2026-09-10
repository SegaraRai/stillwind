import { defineComponent as _defineComponent } from 'vue'
import { style } from "stillwind/react";

import { style as sx } from "stillwind/react";
export const shared=sx('normal');
const __default__ = {name:'Two'};

export default /*@__PURE__*/_defineComponent({
  ...__default__,
  setup(__props, { expose: __expose }) {
  __expose();

const props=style('setup');

const __returned__ = { shared, props }
Object.defineProperty(__returned__, '__isScriptSetup', { enumerable: false, value: true })
return __returned__
}

})