import { Change, MergeView } from "@codemirror/merge";
import { useEffect, useRef } from "react";

import { codeExtensions, type CodeLanguage } from "./CodeView";

export default function DiffView({
  before,
  after,
  language,
  changes,
}: {
  before: string;
  after: string;
  language: CodeLanguage;
  changes?: Array<{ fromA: number; toA: number; fromB: number; toB: number }>;
}) {
  const host = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (!host.current) {
      return undefined;
    }
    const view = new MergeView({
      parent: host.current,
      a: { doc: before, extensions: codeExtensions(language) },
      b: { doc: after, extensions: codeExtensions(language) },
      highlightChanges: true,
      gutter: true,
      collapseUnchanged: { margin: 3, minSize: 8 },
      diffConfig:
        changes === undefined
          ? { scanLimit: 2_000, timeout: 1_000 }
          : {
              override: () =>
                changes.map(
                  (change) => new Change(change.fromA, change.toA, change.fromB, change.toB),
                ),
            },
    });
    return () => view.destroy();
  }, [after, before, changes, language]);
  return <div className="diff" ref={host} />;
}
