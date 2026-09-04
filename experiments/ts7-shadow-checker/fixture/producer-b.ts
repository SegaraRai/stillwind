import { style } from "./style.js";

const scopeB = style.scope();
const refB = style.ref(scopeB);

export { refB as renamedRefB, scopeB };
