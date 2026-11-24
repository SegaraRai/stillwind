---
"@stillwind/cli": patch
"@stillwind/vscode-extension": patch
---

`prefer-canonical-utilities` no longer suggests a replacement that matches only for the current theme values. For example, it no longer suggests `rounded-md` for `rounded-[min(var(--radius-md),10px)]`, because the two differ when `--radius-md` is larger than `10px`.

It also no longer uses the built-in value of `--radius` for a theme value that refers to `var(--radius)`, because the generated CSS does not declare that value. For example, with `--radius-lg: var(--radius)` in `@theme inline` and `--radius` declared in `:root`, it no longer suggests `rounded-lg` for `rounded-[4px]`.
