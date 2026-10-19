import { style } from "stillwind/react";

// Separate calls must agree on the literal `not-prose` class that the `prose` rules exclude.
const article = style("prose");
const excluded = style("not-prose");
document.querySelector("#article")!.className = article.className ?? "";
document.querySelector("#target")!.className = excluded.className ?? "";
