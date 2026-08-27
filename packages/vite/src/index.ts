import type { IncomingMessage, ServerResponse } from "node:http"
import { readFileSync } from "node:fs"
import { compileReact } from "@shorth/native"
import type { Plugin, ViteDevServer } from "vite"

export interface ShorthOptions {
  /** Absolute path to the CSS file containing the project's `@theme` tokens. */
  theme: string
}

interface CompilerOutput {
  css: string
  code: string | null
  classes: Array<{ class_name: string }>
  diagnostics: Array<{ code: string; message: string; start: number; end: number }>
}

const virtualCssId = "virtual:shorth.css"
const resolvedVirtualCssId = `\0${virtualCssId}`

/**
 * Executes the Rust compiler and exposes its aggregate CSS as a Vite virtual
 * stylesheet. There is no runtime utility parser in the client bundle.
 */
export function shorth(options: ShorthOptions): Plugin {
  const cssByModule = new Map<string, string>()

  const compile = (source: string, path: string, theme: string): CompilerOutput => {
    return JSON.parse(compileReact(source, path, theme)) as CompilerOutput
  }

  const loadPlayRequest = (request: IncomingMessage, response: ServerResponse) => {
    let body = ""
    request.on("data", (chunk: Buffer) => { body += chunk.toString() })
    request.on("end", () => {
      try {
        const payload = JSON.parse(body) as { source: string; theme: string }
        const result = compile(payload.source, "Play.tsx", payload.theme)
        response.setHeader("content-type", "application/json")
        response.end(JSON.stringify(result))
      } catch (error) {
        response.statusCode = 400
        response.end(JSON.stringify({ error: error instanceof Error ? error.message : String(error) }))
      }
    })
  }

  return {
    name: "shorth",
    enforce: "pre",
    resolveId(id) {
      return id === virtualCssId ? resolvedVirtualCssId : undefined
    },
    load(id) {
      if (id === resolvedVirtualCssId) return [...cssByModule.values()].join("\n")
      return undefined
    },
    transform(code, id) {
      if (!/\.[cm]?[jt]sx?$/.test(id) || id.includes("node_modules") || !code.includes("shorth/react")) {
        return undefined
      }
      const output = compile(code, id, readFileSync(options.theme, "utf8"))
      if (output.diagnostics.length > 0) {
        const diagnostic = output.diagnostics[0]
        this.error(`${diagnostic.code}: ${diagnostic.message} (${id}:${diagnostic.start})`)
      }
      if (!output.code) return undefined
      cssByModule.set(id, output.css)
      const runtimeImport = output.code.includes("shorthClsx(")
        ? 'import { shorthClsx } from "shorth/react/runtime";\n'
        : ""
      return {
        code: `${runtimeImport}import "${virtualCssId}";\n${output.code}`,
        map: null,
      }
    },
    configureServer(server: ViteDevServer) {
      server.middlewares.use("/__shorth/compile", loadPlayRequest)
    },
  }
}
