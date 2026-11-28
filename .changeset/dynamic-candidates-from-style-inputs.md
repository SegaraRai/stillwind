---
"stillwind": patch
"@stillwind/plugin-iconify": patch
---

Plugin utilities, such as Iconify icons, are now defined for the candidates that style inputs use, instead of for candidate-like text anywhere in a module. An icon is no longer reported as `unknown-utility` when the same file contains a quote inside a regular expression literal, such as `/["']/`. Comments and unrelated strings no longer make the plugin define utilities.
