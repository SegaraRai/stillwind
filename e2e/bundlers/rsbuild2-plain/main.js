import { props } from "./static.js";

document.body.innerHTML =
  '<p id="main">Main</p><button id="load">Load</button><p id="lazy">Lazy</p>';
document.querySelector("#main").className = props.className ?? "";
document.querySelector("#load").onclick = () => import("./lazy.js").then((m) => m.show());
