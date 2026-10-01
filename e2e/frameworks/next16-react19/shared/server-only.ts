import "server-only";

export function serverLabel(): string {
  const secret =
    process.env.STILLWIND_SERVER_SENTINEL ?? "STILLWIND_SERVER_IMPLEMENTATION_SENTINEL";
  return secret.startsWith("STILLWIND_") ? "Server" : "Invalid server value";
}
