import { css } from "@codemirror/lang-css";
import { javascript } from "@codemirror/lang-javascript";
import { EditorState } from "@codemirror/state";
import { oneDark } from "@codemirror/theme-one-dark";
import { EditorView, highlightActiveLineGutter, lineNumbers } from "@codemirror/view";
import { useEffect, useRef } from "react";

export default function CodeView({
  value,
  language,
}: {
  value: string;
  language: "typescript" | "tsx" | "css";
}) {
  const host = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (!host.current) return undefined;
    const view = new EditorView({
      parent: host.current,
      state: EditorState.create({
        doc: value,
        extensions: [
          lineNumbers(),
          highlightActiveLineGutter(),
          language === "css" ? css() : javascript({ typescript: true, jsx: language === "tsx" }),
          oneDark,
          EditorState.readOnly.of(true),
          EditorView.editable.of(false),
          EditorView.lineWrapping,
          EditorView.theme({
            "&": { height: "100%", backgroundColor: "transparent" },
            ".cm-scroller": { fontFamily: "var(--mono)", lineHeight: "1.65" },
            ".cm-gutters": { backgroundColor: "transparent", border: "none", color: "#52617a" },
            ".cm-content": { padding: "14px 0 40px" },
          }),
        ],
      }),
    });
    return () => view.destroy();
  }, [language, value]);
  return <div className="code" ref={host} />;
}
