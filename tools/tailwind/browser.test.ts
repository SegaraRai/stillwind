import { spawn } from "node:child_process";
import { existsSync, mkdirSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";
import { setTimeout } from "node:timers/promises";
import { pathToFileURL } from "node:url";

import { expect, test } from "vite-plus/test";

import { compileParity } from "./comparison.ts";
import { compileTailwindCss } from "./compiler.ts";
import { stillwindFixtureCss } from "./stillwind-css.ts";

test("generated CSS preserves browser cascade behavior", async () => {
  const root = resolve(import.meta.dirname, "../..");
  const output = resolve(root, "target/tailwind-browser-contract");
  mkdirSync(output, { recursive: true });
  const browser =
    process.env.STILLWIND_BROWSER ?? "C:/Program Files (x86)/Microsoft/Edge/Application/msedge.exe";
  expect(existsSync(browser), "Set STILLWIND_BROWSER to a Chromium/Edge executable").toBeTruthy();
  const baseInput =
    '@import "tailwindcss/theme" layer(theme);@import "tailwindcss/utilities" layer(utilities);';
  const cases = [
    {
      name: "arbitrary shadow composes with blur and color",
      candidates: ["blur-sm", "drop-shadow-[0_1px_2px_rgb(0_0_0/0.25)]", "drop-shadow-red-500"],
      properties: ["filter"],
    },
    {
      name: "transform-cpu inherits unregistered input",
      candidates: ["transform-cpu"],
      properties: ["transform"],
      parent: "--tw-rotate-x:rotateX(30deg)",
    },
    {
      name: "transform-gpu inherits unregistered input",
      candidates: ["transform-gpu"],
      properties: ["transform"],
      parent: "--tw-rotate-x:rotateX(30deg)",
    },
    {
      name: "transform explicitly registers its inputs",
      candidates: ["transform"],
      properties: ["transform"],
      parent: "--tw-rotate-x:rotateX(30deg)",
    },
    {
      name: "ring-offset alone preserves inherited color",
      candidates: ["ring-offset-2"],
      properties: ["--tw-ring-offset-color"],
      parent: "--tw-ring-offset-color:red",
    },
    {
      name: "ring and ring-offset activate support together",
      candidates: ["ring-2", "ring-offset-2"],
      properties: ["box-shadow"],
    },
    {
      name: "theme slot can be overridden by a later layer",
      candidates: ["bg-brand"],
      properties: ["background-color"],
      input: "@theme {--color-brand:red;}",
      extra: "@layer overrides{:root{--color-brand:green}}",
    },
    {
      name: "mask initial values survive fallback omission",
      candidates: ["mask-b-from-0"],
      properties: ["mask-image", "mask-composite"],
    },
    {
      name: "empty blur remains an empty value",
      candidates: ["blur-none", "brightness-50"],
      properties: ["filter"],
    },
    {
      name: "theme keyframes deliberately allow layered overrides",
      candidates: ["animate-spin"],
      properties: ["opacity", "transform"],
      extra: "@layer overrides{@keyframes spin{from{opacity:1}to{opacity:0}}}",
      different: true,
    },
  ];
  const payload = [];
  for (const entry of cases) {
    const input = baseInput + (entry.input ?? "");
    const expected = await compileTailwindCss({
      css: input,
      candidates: entry.candidates,
      base: resolve(root, "fixtures/tailwind-v4"),
    });
    const native = compileParity(
      `import {style} from 'stillwind/react';style(${JSON.stringify(entry.candidates.join(" "))});`,
      stillwindFixtureCss(input),
    );
    expect(native.diagnostics.length).toBe(0);
    payload.push({
      ...entry,
      expected,
      actual: native.css,
      expectedClass: entry.candidates.join(" "),
      actualClass: native.classes[0].className,
    });
  }
  const html = `<!doctype html><body><script>
const cases=${JSON.stringify(payload).replaceAll("<", "\\u003c")};
async function read(c,side){
 const frame=document.createElement('iframe');document.body.append(frame);
 const ready=new Promise(resolve=>frame.onload=resolve);
 frame.srcdoc='<style>'+c[side]+(c.extra||'')+'</style><div id="parent"><div id="probe"></div></div>';
 await ready;
 const doc=frame.contentDocument, el=doc.getElementById('probe');
 doc.getElementById('parent').style.cssText=c.parent||'';
 el.className=c[side+'Class'];
 for(const animation of el.getAnimations()){animation.pause();animation.currentTime=500;}
 const style=frame.contentWindow.getComputedStyle(el);
 const values=c.properties.map(name=>style.getPropertyValue(name));
 let pixels=[];
 if(c.properties.includes('filter')){
   const canvas=doc.createElement('canvas');canvas.width=128;canvas.height=128;
   const context=canvas.getContext('2d');context.filter=style.filter;
   context.fillStyle='white';context.fillRect(48,48,24,24);
   pixels=Array.from(context.getImageData(0,0,128,128).data);
 }
 frame.remove();return {values,pixels};
}
(async()=>{const results=[];for(const c of cases){
 const expected=await read(c,'expected'),actual=await read(c,'actual');
 // Computed colors may serialize in oklab vs oklch. Compare filter rendering,
 // allowing one 8-bit channel step for color-conversion rounding, not CSS strings.
 const maxPixelError=expected.pixels.reduce((max,value,i)=>Math.max(max,Math.abs(value-actual.pixels[i])),0);
 const passed=c.different ? expected.values[0]==='1'&&actual.values[0]==='0.5' : expected.pixels.length ? maxPixelError<=1 : JSON.stringify(expected.values)===JSON.stringify(actual.values);
 results.push({name:c.name,expected:expected.values,actual:actual.values,maxPixelError,passed});
}document.body.textContent=JSON.stringify(results);document.body.dataset.status=results.every(r=>r.passed)?'passed':'failed';})();
</script>`;
  const page = resolve(output, "index.html");
  writeFileSync(page, html);
  const profile = mkdtempSync(resolve(output, "profile-"));
  const child = spawn(
    browser,
    [
      "--headless=new",
      "--disable-gpu",
      "--no-first-run",
      "--no-default-browser-check",
      `--user-data-dir=${profile}`,
      "--remote-debugging-port=0",
      pathToFileURL(page).href,
    ],
    { windowsHide: true, stdio: "ignore" },
  );
  let socket: WebSocket | undefined;
  try {
    const portFile = resolve(profile, "DevToolsActivePort");
    const deadline = Date.now() + 20000;
    while (!existsSync(portFile) && Date.now() < deadline) {
      await setTimeout(100);
    }
    expect(existsSync(portFile), "headless browser did not publish a debug port").toBeTruthy();
    const port = readFileSync(portFile, "utf8").split(/\r?\n/)[0];
    const targets = (await (await fetch(`http://127.0.0.1:${port}/json/list`)).json()) as Array<{
      url: string;
      webSocketDebuggerUrl: string;
    }>;
    const target = targets.find((target) => target.url === pathToFileURL(page).href);
    expect(target, "test page did not open").toBeTruthy();
    socket = new WebSocket(target!.webSocketDebuggerUrl);
    await new Promise<void>((resolve, reject) => {
      socket!.onopen = () => resolve();
      socket!.onerror = reject;
    });
    let id = 0;
    async function command(
      method: string,
      params: Record<string, unknown> = {},
    ): Promise<{ result?: { value?: string } }> {
      const request = ++id;
      return new Promise((resolve, reject) => {
        const timeout = globalThis.setTimeout(
          () => reject(new Error(`CDP timeout: ${method}`)),
          5000,
        );
        const listener = (event: MessageEvent) => {
          const result = JSON.parse(String(event.data));
          if (result.id !== request) {
            return;
          }
          globalThis.clearTimeout(timeout);
          socket!.removeEventListener("message", listener);
          if (result.error) {
            reject(new Error(JSON.stringify(result.error)));
          } else {
            resolve(result.result);
          }
        };
        socket!.addEventListener("message", listener);
        socket!.send(JSON.stringify({ id: request, method, params }));
      });
    }
    let status = "";
    while (!status && Date.now() < deadline) {
      const result = await command("Runtime.evaluate", {
        expression: "document.body.dataset.status",
        returnByValue: true,
      });
      status = result.result?.value ?? "";
      if (!status) {
        await setTimeout(100);
      }
    }
    const result = await command("Runtime.evaluate", {
      expression: "document.body.textContent",
      returnByValue: true,
    });
    writeFileSync(resolve(output, "result.json"), result.result?.value ?? "");
    await command("Browser.close");
    expect(status, result.result?.value ?? "browser produced no results").toBe("passed");
  } finally {
    socket?.close();
    child.kill();
  }
  console.log(`Browser CSS contracts: ${cases.length}/${cases.length} passed (${browser})`);
}, 60_000);
