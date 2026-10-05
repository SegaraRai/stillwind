import type { ProducerMetadata } from "./compile.ts";

/**
 * Types the graph metadata and resolver options exposed by the workspace's Vite+ backend.
 * Published adapter declarations retain the public Vite API without requiring this backend path.
 */
declare module "vite/rolldown" {
  interface CustomPluginOptions {
    /** Producer source and identities retained across cached Stillwind transforms. */
    stillwind?: ProducerMetadata;
    /** Allows a stylesheet projection to resolve a React Router server-only source. */
    "react-router:dot-server"?: boolean;
  }
}
