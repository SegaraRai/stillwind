export type PlayOutput = import("@stillwind/host").CompilerOutput & {
  previewUrl: string | null;
};

export async function requestCompilation(
  source: string,
  theme: string,
  signal: AbortSignal,
): Promise<PlayOutput> {
  const response = await fetch("/__stillwind/compile", {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ source, theme }),
    signal,
  });
  const output = (await response.json()) as PlayOutput;
  if (!response.ok) {
    throw new Error("error" in output ? String(output.error) : "Preview compilation failed");
  }
  return output;
}
