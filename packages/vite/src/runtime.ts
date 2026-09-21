/** Generates client code for versioned stylesheet updates. */
export function developmentRuntime(
  snapshot: unknown,
  sourceMaps: boolean,
  endpoint: string,
): string {
  return `
${sourceMaps ? `import {GenMapping,addSegment,setSourceContent,toEncodedMap} from ${JSON.stringify(mappingModule)}; import {TraceMap,eachMapping} from ${JSON.stringify(tracingModule)};` : ""}
const key=Symbol.for("stillwind.document");
const state=globalThis[key]??={revision:-1,style:undefined,active:new Set(),snapshot:undefined};
function render(){
  if(!state.snapshot||typeof document==="undefined")return;
  const active=new Set(state.active);
  for(const module of active)for(const dependency of state.snapshot.dependencies[module]??[])active.add(dependency);
  const parts=state.snapshot.parts.filter(part=>!part.producers.length||part.producers.some(producer=>active.has(producer)));
  if(!state.style?.isConnected){state.style=document.createElement("style");state.style.dataset.stillwind="";document.head.append(state.style);}
  let css=parts.map(part=>part.css).join("");
  ${
    sourceMaps
      ? `
  const map=new GenMapping();let line=0,column=0;
  for(const part of parts){
    if(part.map){
      const trace=new TraceMap(part.map);
      eachMapping(trace,m=>{
        const generatedLine=line+m.generatedLine-1;
        const generatedColumn=(m.generatedLine===1?column:0)+m.generatedColumn;
        if(m.source!==null&&m.originalLine!==null)addSegment(map,generatedLine,generatedColumn,m.source,m.originalLine-1,m.originalColumn);
        else addSegment(map,generatedLine,generatedColumn);
      });
      trace.sources.forEach((source,index)=>{if(source!==null&&trace.sourcesContent?.[index]!=null)setSourceContent(map,source,trace.sourcesContent[index]);});
    }
    const lines=part.css.split("\\n");if(lines.length>1){line+=lines.length-1;column=lines.at(-1).length;}else column+=part.css.length;
  }
  css+="\\n/*# sourceMappingURL=data:application/json;base64,"+btoa(unescape(encodeURIComponent(JSON.stringify(toEncodedMap(map)))))+" */";
  `
      : ""
  }
  state.style.textContent=css;
}
export function register(id){state.active.add(id);render();}
export function unregister(id){state.active.delete(id);render();}
export async function synchronize(revision){
  if(typeof document==="undefined")return;
  while(state.revision<revision){
    state.pending??=fetch(${JSON.stringify(endpoint)}).then(response=>{if(!response.ok)throw new Error("Stillwind stylesheet synchronization failed");return response.json();}).then(apply).finally(()=>{state.pending=undefined;});
    await state.pending;
  }
}
function apply(update){if(update.revision<state.revision)return;state.revision=update.revision;state.snapshot=update;render();}
apply(${JSON.stringify(snapshot)});
if(import.meta.hot){import.meta.hot.on("stillwind:css",apply);import.meta.hot.accept();import.meta.hot.dispose(()=>import.meta.hot.off("stillwind:css",apply));}
`;
}
import { fileURLToPath } from "node:url";

const mappingModule = fileURLToPath(import.meta.resolve("@jridgewell/gen-mapping")).replaceAll(
  "\\",
  "/",
);
const tracingModule = fileURLToPath(import.meta.resolve("@jridgewell/trace-mapping")).replaceAll(
  "\\",
  "/",
);
