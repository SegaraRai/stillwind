import { style } from "stillwind/react";

export const item = style("in-card:flex");
export const root = style("card-root:flex");
export const nested = style("nested-card:flex");
export const sheet = style.css({ "in-card:": { "[.label]:": "flex" } });
