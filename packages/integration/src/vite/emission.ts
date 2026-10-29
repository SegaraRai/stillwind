import { createHash } from "node:crypto";

import type { NativeStylesheetPart } from "@stillwind/native";
import { cssBanners } from "stillwind/css";

/**
 * Packs adjacent occurrences with identical ordered producer lists without moving any rule.
 *
 * @param parts - Native partitions in stylesheet order. Input records and source-map arrays are unchanged.
 * @returns New partitions with concatenated CSS, shifted UTF-8 source-map offsets, and content hashes for merged parts.
 */
export function partitionStylesheet(
  parts: readonly NativeStylesheetPart[],
): NativeStylesheetPart[] {
  const output: NativeStylesheetPart[] = [];
  for (const part of parts) {
    const previous = output.at(-1);
    if (previous && JSON.stringify(previous.producers) === JSON.stringify(part.producers)) {
      const byteOffset = Buffer.byteLength(previous.css);
      const sourceOffset = previous.sourceMap.sources.length;
      previous.sourceMap.sources.push(...part.sourceMap.sources);
      previous.sourceMap.entries.push(
        ...part.sourceMap.entries.map((entry) => ({
          ...entry,
          source: entry.source + sourceOffset,
          generatedStart: entry.generatedStart + byteOffset,
          generatedEnd: entry.generatedEnd + byteOffset,
        })),
      );
      previous.css += part.css;
      previous.banners = cssBanners([...previous.banners, ...part.banners]);
      previous.key = createHash("sha256")
        .update(previous.css)
        .update(JSON.stringify(previous.banners))
        .digest("hex");
    } else {
      output.push({
        ...part,
        sourceMap: {
          sources: [...part.sourceMap.sources],
          entries: [...part.sourceMap.entries],
        },
      });
    }
  }
  return output;
}
