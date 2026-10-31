import { style } from "stillwind/react";

export const fileIcon = style({
  "data-[type=stillwind]:": "icon-stillwind--mark",
  "data-[type=javascriptreact]:": "icon-file--react text-file-javascriptreact",
  "data-[type=typescriptreact]:": "icon-file--react text-file-typescriptreact",
  "data-[type=javascript]:": "icon-file--js text-file-javascript",
  "data-[type=typescript]:": "icon-file--ts text-file-typescript",
  "data-[type=astro]:": "icon-simple-icons--astro text-file-astro",
  "data-[type=svelte]:": "icon-simple-icons--svelte text-file-svelte",
  "data-[type=vue]:": "icon-simple-icons--vuedotjs text-file-vue",
  "data-[type=css]:": "icon-file--css text-file-css",
  "data-[type=svg]:": "icon-file--svg text-file-svg",
  "data-[type=image]:": "icon-lucide--image text-file-image",
  "data-[type=html]:": "icon-file--html text-file-html",
  "data-[type=tsconfig]:": "icon-simple-icons--typescript text-file-typescript",
  "data-[type=json]:": "icon-file--json text-file-json",
  "data-[type=markdown]:": "icon-simple-icons--markdown text-file-markdown",
  "data-[type=plaintext]:": "icon-lucide--file-text text-file-plaintext",
  "data-[type=unknown]:": "icon-lucide--file text-subtle",
});

export interface FileInfo {
  type: string;
  label: string;
}

export function getFileInfo(filename: string): FileInfo {
  if (/(?:^|\/)stillwind\.config\.(?:[cm]?[jt]s|json)$/i.test(filename)) {
    return {
      type: "stillwind",
      label: "Stillwind Config",
    };
  }
  if (/\.[cm]?jsx$/i.test(filename)) {
    return {
      type: "javascriptreact",
      label: "JavaScript JSX",
    };
  }
  if (/\.[cm]?tsx$/i.test(filename)) {
    return {
      type: "typescriptreact",
      label: "TypeScript JSX",
    };
  }
  if (/\.[cm]?js$/i.test(filename)) {
    return {
      type: "javascript",
      label: "JavaScript",
    };
  }
  if (/\.[cm]?ts$/i.test(filename)) {
    return {
      type: "typescript",
      label: "TypeScript",
    };
  }
  if (/\.astro$/i.test(filename)) {
    return {
      type: "astro",
      label: "Astro",
    };
  }
  if (/\.svelte$/i.test(filename)) {
    return {
      type: "svelte",
      label: "Svelte",
    };
  }
  if (/\.vue$/i.test(filename)) {
    return {
      type: "vue",
      label: "Vue",
    };
  }
  if (/\.css$/i.test(filename)) {
    return {
      type: "css",
      label: "CSS",
    };
  }
  if (/\.(jpe?g|png|gif|webp|avif|hei[cf]|jxl)$/i.test(filename)) {
    return {
      type: "image",
      label: "Image",
    };
  }
  if (/\.svg$/i.test(filename)) {
    return {
      type: "svg",
      label: "SVG",
    };
  }
  if (/\.html$/i.test(filename)) {
    return {
      type: "html",
      label: "HTML",
    };
  }
  if (/(?:^|\/)tsconfig\.json$/i.test(filename)) {
    return {
      type: "tsconfig",
      label: "TypeScript Config",
    };
  }
  if (/\.json$/i.test(filename)) {
    return {
      type: "json",
      label: "JSON",
    };
  }
  if (/\.md$/i.test(filename)) {
    return {
      type: "markdown",
      label: "Markdown",
    };
  }
  if (/\.te?xt$/i.test(filename)) {
    return {
      type: "plaintext",
      label: "Plain Text",
    };
  }
  return {
    type: "unknown",
    label: "Unknown",
  };
}
