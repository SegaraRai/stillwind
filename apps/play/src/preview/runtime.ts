import * as React from "react";
import * as ReactDOM from "react-dom";
import Refresh from "react-refresh/runtime";
import * as JSXDev from "react/jsx-dev-runtime";
import * as JSX from "react/jsx-runtime";

import { installSandboxGuards } from "./sandbox.ts";

installSandboxGuards(window);

Refresh.injectIntoGlobalHook(window);

const Client = require("react-dom/client") as typeof import("react-dom/client");

const modules: Record<string, unknown> = {
  react: React,
  "react-dom": ReactDOM,
  "react-dom/client": Client,
  "react/jsx-runtime": JSX,
  "react/jsx-dev-runtime": JSXDev,
};
const root = Client.createRoot(document.getElementById("root")!);

let lastCode = "";
let current: unknown;
let mounted = false;
let revision = -1;
let port: MessagePort | undefined;

Object.assign(window, {
  require: (name: string) => {
    if (!(name in modules)) {
      throw new Error(`Preview runtime module is unavailable: ${name}`);
    }
    return modules[name];
  },
  $RefreshReg$: (type: unknown, id: string) => {
    Refresh.register(type, id);
  },
  $RefreshSig$: Refresh.createSignatureFunctionForTransform,
});

const report = (error: unknown): void => {
  port?.postMessage({
    error: error instanceof Error ? error.message : String(error),
  });
};

window.addEventListener("error", (event): void => {
  report(event.error ?? event.message);
});

window.addEventListener("unhandledrejection", (event): void => {
  report(event.reason);
});

window.addEventListener("message", (event): void => {
  if (
    event.source !== parent ||
    event.data !== "stillwind-preview-connect" ||
    !event.ports[0] ||
    port
  ) {
    return;
  }

  port = event.ports[0];

  port.onmessage = ({
    data,
  }: MessageEvent<{
    revision: number;
    code: string;
    css: string;
  }>) => {
    if (data.revision < revision) {
      return;
    }

    revision = data.revision;
    document.getElementById("styles")!.textContent = data.css;
    try {
      if (data.code !== lastCode) {
        // Compiled application code executes inside the iframe's opaque origin.
        // oxlint-disable-next-line typescript/no-implied-eval
        const exports = new Function(`${data.code}\nreturn StillwindApplication;`)() as {
          App?: React.ComponentType;
          default?: React.ComponentType;
        };
        const component = exports.App ?? exports.default;
        if (!component) {
          throw new Error("Component.tsx must export App or a default React component");
        }
        if (!mounted || !Refresh.isLikelyComponentType(current)) {
          root.render(React.createElement(component));
        } else {
          Refresh.performReactRefresh();
          root.render(React.createElement(component));
        }
        mounted = true;
        current = component;
        lastCode = data.code;
      }
      port?.postMessage({
        revision,
        ready: true,
      });
    } catch (error) {
      report(error);
    }
  };

  port.postMessage({
    connected: true,
  });
});

parent.postMessage("stillwind-preview-ready", "*");
