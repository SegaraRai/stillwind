/** Immutable string fragments remain reconstructible in isolated loader workers. */
export default function webpackFragmentLoader(this: any): string {
  const encoded = new URLSearchParams(this.resourceQuery).get("css");
  if (!encoded) {
    throw new Error("Missing Stillwind CSS fragment");
  }
  const { css, resources } = JSON.parse(Buffer.from(encoded, "base64url").toString("utf8"));
  const imports = resources.map(
    (resource: { id: string; placeholder: string }, index: number) =>
      `import asset${index} from ${JSON.stringify(resource.id)};\n`,
  );
  const replacements = resources
    .map(
      (resource: { id: string; placeholder: string }, index: number) =>
        `.replaceAll(${JSON.stringify(resource.placeholder)}, JSON.stringify(typeof asset${index} === "string" ? asset${index} : asset${index}.src).slice(1, -1))`,
    )
    .join("");
  return [
    "// Stillwind CSS fragment (webpack)\n",
    ...imports,
    "\n",
    `export const css = ${JSON.stringify(css)}${replacements};\n`,
  ].join("");
}
