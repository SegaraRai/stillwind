import css from "stillwind/module-css";
import { style } from "stillwind/react";

import { automatic } from "./automatic";

Object.assign(globalThis, { css, automatic, props: style("tone current picture") });
