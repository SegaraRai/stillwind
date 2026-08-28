# Exhaustive direct built-in registrations

## `builtins.static-utilities`

### Input CSS

```css
@import "tailwindcss/theme" layer(theme);
@import "tailwindcss/utilities" layer(utilities);
```

### Candidates

```text
-translate-full absolute accent-auto align-baseline align-bottom align-middle align-sub align-super align-text-bottom align-text-top
align-top antialiased appearance-auto appearance-none backface-hidden backface-visible basis-auto basis-full bg-auto bg-bottom
bg-bottom-left bg-bottom-right bg-center bg-clip-border bg-clip-content bg-clip-padding bg-clip-text bg-contain bg-cover bg-fixed
bg-left bg-local bg-no-repeat bg-none bg-origin-border bg-origin-content bg-origin-padding bg-repeat bg-repeat-round bg-repeat-space
bg-repeat-x bg-repeat-y bg-right bg-scroll bg-top bg-top-left bg-top-right block border-collapse border-dashed
border-dotted border-double border-hidden border-none border-separate border-solid box-border box-content box-decoration-clone box-decoration-slice
break-all break-keep break-normal capitalize caption-bottom caption-top clear-both clear-end clear-left clear-none
clear-right clear-start collapse contain-content contain-inline-size contain-layout contain-none contain-paint contain-size contain-strict
contain-style content-around content-baseline content-between content-center content-center-safe content-end content-end-safe content-evenly content-none
content-normal content-start content-stretch contents decoration-auto decoration-dashed decoration-dotted decoration-double decoration-from-font decoration-solid
decoration-wavy diagonal-fractions divide-x-reverse divide-y-reverse drop-shadow-none duration-initial field-sizing-content field-sizing-fixed fill-none fixed
flex flex-auto flex-col flex-col-reverse flex-initial flex-none flex-nowrap flex-row flex-row-reverse flex-wrap
flex-wrap-reverse float-end float-left float-none float-right float-start flow-root font-stretch-condensed font-stretch-expanded font-stretch-extra-condensed
font-stretch-extra-expanded font-stretch-normal font-stretch-semi-condensed font-stretch-semi-expanded font-stretch-ultra-condensed font-stretch-ultra-expanded forced-color-adjust-auto forced-color-adjust-none grid grid-flow-col
grid-flow-col-dense grid-flow-dense grid-flow-row grid-flow-row-dense hidden hyphens-auto hyphens-manual hyphens-none inline inline-block
inline-flex inline-grid inline-table inset-shadow-initial invisible isolate isolation-auto italic items-baseline items-baseline-last
items-center items-center-safe items-end items-end-safe items-start items-stretch justify-around justify-baseline justify-between justify-center
justify-center-safe justify-end justify-end-safe justify-evenly justify-items-center justify-items-center-safe justify-items-end justify-items-end-safe justify-items-normal justify-items-start
justify-items-stretch justify-normal justify-self-auto justify-self-center justify-self-center-safe justify-self-end justify-self-end-safe justify-self-start justify-self-stretch justify-start
justify-stretch line-through lining-nums list-inside list-item list-outside lowercase mask-add mask-alpha mask-auto
mask-bottom mask-bottom-left mask-bottom-right mask-center mask-circle mask-clip-border mask-clip-content mask-clip-fill mask-clip-padding mask-clip-stroke
mask-clip-view mask-contain mask-cover mask-ellipse mask-exclude mask-intersect mask-left mask-luminance mask-match mask-no-clip
mask-no-repeat mask-none mask-origin-border mask-origin-content mask-origin-fill mask-origin-padding mask-origin-stroke mask-origin-view mask-radial-at-bottom mask-radial-at-bottom-left
mask-radial-at-bottom-right mask-radial-at-center mask-radial-at-left mask-radial-at-right mask-radial-at-top mask-radial-at-top-left mask-radial-at-top-right mask-radial-closest-corner mask-radial-closest-side mask-radial-farthest-corner
mask-radial-farthest-side mask-repeat mask-repeat-round mask-repeat-space mask-repeat-x mask-repeat-y mask-right mask-subtract mask-top mask-top-left
mask-top-right mask-type-alpha mask-type-luminance mix-blend-plus-darker mix-blend-plus-lighter no-underline normal-case normal-nums not-italic not-sr-only
object-contain object-cover object-fill object-none object-scale-down oldstyle-nums ordinal outline-dashed outline-dotted outline-double
outline-none outline-solid overline place-content-around place-content-baseline place-content-between place-content-center place-content-center-safe place-content-end place-content-end-safe
place-content-evenly place-content-start place-content-stretch place-items-baseline place-items-center place-items-center-safe place-items-end place-items-end-safe place-items-start place-items-stretch
place-self-auto place-self-center place-self-center-safe place-self-end place-self-end-safe place-self-start place-self-stretch pointer-events-auto pointer-events-none proportional-nums
relative resize resize-none resize-x resize-y ring-inset rotate-none scale-3d scale-none scheme-dark
scheme-light scheme-light-dark scheme-normal scheme-only-dark scheme-only-light scroll-auto scroll-smooth scrollbar-auto scrollbar-gutter-auto scrollbar-gutter-both
scrollbar-gutter-stable scrollbar-none scrollbar-thin self-auto self-baseline self-baseline-last self-center self-center-safe self-end self-end-safe
self-start self-stretch shadow-initial slashed-zero snap-align-none snap-always snap-center snap-end snap-mandatory snap-none
snap-normal snap-proximity snap-start space-x-reverse space-y-reverse sr-only stacked-fractions static sticky stroke-none
subpixel-antialiased table table-auto table-caption table-cell table-column table-column-group table-fixed table-footer-group table-header-group
table-row table-row-group tabular-nums text-balance text-center text-clip text-ellipsis text-end text-justify text-left
text-nowrap text-pretty text-right text-shadow-initial text-start text-wrap touch-pinch-zoom transform-3d transform-border transform-content
transform-cpu transform-fill transform-flat transform-gpu transform-none transform-stroke transform-view transition-discrete transition-normal translate-3d
translate-full translate-none truncate underline uppercase via-none visible whitespace-break-spaces whitespace-normal whitespace-nowrap
whitespace-pre whitespace-pre-line whitespace-pre-wrap will-change-auto will-change-contents will-change-scroll will-change-transform wrap-anywhere wrap-break-word wrap-normal
```

### Expected CSS

```css
/*! tailwindcss v4.3.3 | MIT License | https://tailwindcss.com */
@layer properties;
@layer utilities {
  .pointer-events-auto {
    pointer-events: auto;
  }
  .pointer-events-none {
    pointer-events: none;
  }
  .collapse {
    visibility: collapse;
  }
  .invisible {
    visibility: hidden;
  }
  .visible {
    visibility: visible;
  }
  .sr-only {
    position: absolute;
    width: 1px;
    height: 1px;
    padding: 0;
    margin: -1px;
    overflow: hidden;
    clip-path: inset(50%);
    white-space: nowrap;
    border-width: 0;
  }
  .not-sr-only {
    position: static;
    width: auto;
    height: auto;
    padding: 0;
    margin: 0;
    overflow: visible;
    clip-path: none;
    white-space: normal;
  }
  .absolute {
    position: absolute;
  }
  .fixed {
    position: fixed;
  }
  .relative {
    position: relative;
  }
  .static {
    position: static;
  }
  .sticky {
    position: sticky;
  }
  .isolate {
    isolation: isolate;
  }
  .isolation-auto {
    isolation: auto;
  }
  .float-end {
    float: inline-end;
  }
  .float-left {
    float: left;
  }
  .float-none {
    float: none;
  }
  .float-right {
    float: right;
  }
  .float-start {
    float: inline-start;
  }
  .clear-both {
    clear: both;
  }
  .clear-end {
    clear: inline-end;
  }
  .clear-left {
    clear: left;
  }
  .clear-none {
    clear: none;
  }
  .clear-right {
    clear: right;
  }
  .clear-start {
    clear: inline-start;
  }
  .box-border {
    box-sizing: border-box;
  }
  .box-content {
    box-sizing: content-box;
  }
  .block {
    display: block;
  }
  .contents {
    display: contents;
  }
  .flex {
    display: flex;
  }
  .flow-root {
    display: flow-root;
  }
  .grid {
    display: grid;
  }
  .hidden {
    display: none;
  }
  .inline {
    display: inline;
  }
  .inline-block {
    display: inline-block;
  }
  .inline-flex {
    display: inline-flex;
  }
  .inline-grid {
    display: inline-grid;
  }
  .inline-table {
    display: inline-table;
  }
  .list-item {
    display: list-item;
  }
  .table {
    display: table;
  }
  .table-caption {
    display: table-caption;
  }
  .table-cell {
    display: table-cell;
  }
  .table-column {
    display: table-column;
  }
  .table-column-group {
    display: table-column-group;
  }
  .table-footer-group {
    display: table-footer-group;
  }
  .table-header-group {
    display: table-header-group;
  }
  .table-row {
    display: table-row;
  }
  .table-row-group {
    display: table-row-group;
  }
  .field-sizing-content {
    field-sizing: content;
  }
  .field-sizing-fixed {
    field-sizing: fixed;
  }
  .flex-auto {
    flex: auto;
  }
  .flex-initial {
    flex: 0 auto;
  }
  .flex-none {
    flex: none;
  }
  .basis-auto {
    flex-basis: auto;
  }
  .basis-full {
    flex-basis: 100%;
  }
  .table-auto {
    table-layout: auto;
  }
  .table-fixed {
    table-layout: fixed;
  }
  .caption-bottom {
    caption-side: bottom;
  }
  .caption-top {
    caption-side: top;
  }
  .border-collapse {
    border-collapse: collapse;
  }
  .border-separate {
    border-collapse: separate;
  }
  .-translate-full {
    --tw-translate-x: -100%;
    --tw-translate-y: -100%;
    translate: var(--tw-translate-x) var(--tw-translate-y);
  }
  .translate-full {
    --tw-translate-x: 100%;
    --tw-translate-y: 100%;
    translate: var(--tw-translate-x) var(--tw-translate-y);
  }
  .translate-3d {
    translate: var(--tw-translate-x) var(--tw-translate-y) var(--tw-translate-z);
  }
  .translate-none {
    translate: none;
  }
  .scale-3d {
    scale: var(--tw-scale-x) var(--tw-scale-y) var(--tw-scale-z);
  }
  .scale-none {
    scale: none;
  }
  .rotate-none {
    rotate: none;
  }
  .transform-cpu {
    transform: var(--tw-rotate-x,) var(--tw-rotate-y,) var(--tw-rotate-z,) var(--tw-skew-x,) var(--tw-skew-y,);
  }
  .transform-gpu {
    transform: translateZ(0) var(--tw-rotate-x,) var(--tw-rotate-y,) var(--tw-rotate-z,) var(--tw-skew-x,) var(--tw-skew-y,);
  }
  .transform-none {
    transform: none;
  }
  .touch-pinch-zoom {
    --tw-pinch-zoom: pinch-zoom;
    touch-action: var(--tw-pan-x,) var(--tw-pan-y,) var(--tw-pinch-zoom,);
  }
  .resize {
    resize: both;
  }
  .resize-none {
    resize: none;
  }
  .resize-x {
    resize: horizontal;
  }
  .resize-y {
    resize: vertical;
  }
  .snap-none {
    scroll-snap-type: none;
  }
  .snap-mandatory {
    --tw-scroll-snap-strictness: mandatory;
  }
  .snap-proximity {
    --tw-scroll-snap-strictness: proximity;
  }
  .snap-align-none {
    scroll-snap-align: none;
  }
  .snap-center {
    scroll-snap-align: center;
  }
  .snap-end {
    scroll-snap-align: end;
  }
  .snap-start {
    scroll-snap-align: start;
  }
  .snap-always {
    scroll-snap-stop: always;
  }
  .snap-normal {
    scroll-snap-stop: normal;
  }
  .scrollbar-auto {
    scrollbar-width: auto;
  }
  .scrollbar-none {
    scrollbar-width: none;
  }
  .scrollbar-thin {
    scrollbar-width: thin;
  }
  .scrollbar-gutter-auto {
    scrollbar-gutter: auto;
  }
  .scrollbar-gutter-both {
    scrollbar-gutter: stable both-edges;
  }
  .scrollbar-gutter-stable {
    scrollbar-gutter: stable;
  }
  .list-inside {
    list-style-position: inside;
  }
  .list-outside {
    list-style-position: outside;
  }
  .appearance-auto {
    appearance: auto;
  }
  .appearance-none {
    appearance: none;
  }
  .grid-flow-col {
    grid-auto-flow: column;
  }
  .grid-flow-col-dense {
    grid-auto-flow: column dense;
  }
  .grid-flow-dense {
    grid-auto-flow: dense;
  }
  .grid-flow-row {
    grid-auto-flow: row;
  }
  .grid-flow-row-dense {
    grid-auto-flow: row dense;
  }
  .flex-col {
    flex-direction: column;
  }
  .flex-col-reverse {
    flex-direction: column-reverse;
  }
  .flex-row {
    flex-direction: row;
  }
  .flex-row-reverse {
    flex-direction: row-reverse;
  }
  .flex-nowrap {
    flex-wrap: nowrap;
  }
  .flex-wrap {
    flex-wrap: wrap;
  }
  .flex-wrap-reverse {
    flex-wrap: wrap-reverse;
  }
  .place-content-around {
    place-content: space-around;
  }
  .place-content-baseline {
    place-content: baseline;
  }
  .place-content-between {
    place-content: space-between;
  }
  .place-content-center {
    place-content: center;
  }
  .place-content-center-safe {
    place-content: safe center;
  }
  .place-content-end {
    place-content: end;
  }
  .place-content-end-safe {
    place-content: safe end;
  }
  .place-content-evenly {
    place-content: space-evenly;
  }
  .place-content-start {
    place-content: start;
  }
  .place-content-stretch {
    place-content: stretch;
  }
  .place-items-baseline {
    place-items: baseline;
  }
  .place-items-center {
    place-items: center;
  }
  .place-items-center-safe {
    place-items: safe center;
  }
  .place-items-end {
    place-items: end;
  }
  .place-items-end-safe {
    place-items: safe end;
  }
  .place-items-start {
    place-items: start;
  }
  .place-items-stretch {
    place-items: stretch;
  }
  .content-around {
    align-content: space-around;
  }
  .content-baseline {
    align-content: baseline;
  }
  .content-between {
    align-content: space-between;
  }
  .content-center {
    align-content: center;
  }
  .content-center-safe {
    align-content: safe center;
  }
  .content-end {
    align-content: flex-end;
  }
  .content-end-safe {
    align-content: safe flex-end;
  }
  .content-evenly {
    align-content: space-evenly;
  }
  .content-normal {
    align-content: normal;
  }
  .content-start {
    align-content: flex-start;
  }
  .content-stretch {
    align-content: stretch;
  }
  .items-baseline {
    align-items: baseline;
  }
  .items-baseline-last {
    align-items: last baseline;
  }
  .items-center {
    align-items: center;
  }
  .items-center-safe {
    align-items: safe center;
  }
  .items-end {
    align-items: flex-end;
  }
  .items-end-safe {
    align-items: safe flex-end;
  }
  .items-start {
    align-items: flex-start;
  }
  .items-stretch {
    align-items: stretch;
  }
  .justify-around {
    justify-content: space-around;
  }
  .justify-baseline {
    justify-content: baseline;
  }
  .justify-between {
    justify-content: space-between;
  }
  .justify-center {
    justify-content: center;
  }
  .justify-center-safe {
    justify-content: safe center;
  }
  .justify-end {
    justify-content: flex-end;
  }
  .justify-end-safe {
    justify-content: safe flex-end;
  }
  .justify-evenly {
    justify-content: space-evenly;
  }
  .justify-normal {
    justify-content: normal;
  }
  .justify-start {
    justify-content: flex-start;
  }
  .justify-stretch {
    justify-content: stretch;
  }
  .justify-items-center {
    justify-items: center;
  }
  .justify-items-center-safe {
    justify-items: safe center;
  }
  .justify-items-end {
    justify-items: end;
  }
  .justify-items-end-safe {
    justify-items: safe end;
  }
  .justify-items-normal {
    justify-items: normal;
  }
  .justify-items-start {
    justify-items: start;
  }
  .justify-items-stretch {
    justify-items: stretch;
  }
  :where(.space-y-reverse > :not(:last-child)) {
    --tw-space-y-reverse: 1;
  }
  :where(.space-x-reverse > :not(:last-child)) {
    --tw-space-x-reverse: 1;
  }
  :where(.divide-y-reverse > :not(:last-child)) {
    --tw-divide-y-reverse: 1;
  }
  .place-self-auto {
    place-self: auto;
  }
  .place-self-center {
    place-self: center;
  }
  .place-self-center-safe {
    place-self: safe center;
  }
  .place-self-end {
    place-self: end;
  }
  .place-self-end-safe {
    place-self: safe end;
  }
  .place-self-start {
    place-self: start;
  }
  .place-self-stretch {
    place-self: stretch;
  }
  .self-auto {
    align-self: auto;
  }
  .self-baseline {
    align-self: baseline;
  }
  .self-baseline-last {
    align-self: last baseline;
  }
  .self-center {
    align-self: center;
  }
  .self-center-safe {
    align-self: safe center;
  }
  .self-end {
    align-self: flex-end;
  }
  .self-end-safe {
    align-self: safe flex-end;
  }
  .self-start {
    align-self: flex-start;
  }
  .self-stretch {
    align-self: stretch;
  }
  .justify-self-auto {
    justify-self: auto;
  }
  .justify-self-center {
    justify-self: center;
  }
  .justify-self-center-safe {
    justify-self: safe center;
  }
  .justify-self-end {
    justify-self: flex-end;
  }
  .justify-self-end-safe {
    justify-self: safe flex-end;
  }
  .justify-self-start {
    justify-self: flex-start;
  }
  .justify-self-stretch {
    justify-self: stretch;
  }
  .truncate {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .scroll-auto {
    scroll-behavior: auto;
  }
  .scroll-smooth {
    scroll-behavior: smooth;
  }
  .border-dashed {
    --tw-border-style: dashed;
    border-style: dashed;
  }
  .border-dotted {
    --tw-border-style: dotted;
    border-style: dotted;
  }
  .border-double {
    --tw-border-style: double;
    border-style: double;
  }
  .border-hidden {
    --tw-border-style: hidden;
    border-style: hidden;
  }
  .border-none {
    --tw-border-style: none;
    border-style: none;
  }
  .border-solid {
    --tw-border-style: solid;
    border-style: solid;
  }
  .bg-none {
    background-image: none;
  }
  .via-none {
    --tw-gradient-via-stops: initial;
  }
  .mask-none {
    mask-image: none;
  }
  .mask-circle {
    --tw-mask-radial-shape: circle;
  }
  .mask-ellipse {
    --tw-mask-radial-shape: ellipse;
  }
  .mask-radial-closest-corner {
    --tw-mask-radial-size: closest-corner;
  }
  .mask-radial-closest-side {
    --tw-mask-radial-size: closest-side;
  }
  .mask-radial-farthest-corner {
    --tw-mask-radial-size: farthest-corner;
  }
  .mask-radial-farthest-side {
    --tw-mask-radial-size: farthest-side;
  }
  .mask-radial-at-bottom {
    --tw-mask-radial-position: bottom;
  }
  .mask-radial-at-bottom-left {
    --tw-mask-radial-position: bottom left;
  }
  .mask-radial-at-bottom-right {
    --tw-mask-radial-position: bottom right;
  }
  .mask-radial-at-center {
    --tw-mask-radial-position: center;
  }
  .mask-radial-at-left {
    --tw-mask-radial-position: left;
  }
  .mask-radial-at-right {
    --tw-mask-radial-position: right;
  }
  .mask-radial-at-top {
    --tw-mask-radial-position: top;
  }
  .mask-radial-at-top-left {
    --tw-mask-radial-position: top left;
  }
  .mask-radial-at-top-right {
    --tw-mask-radial-position: top right;
  }
  .box-decoration-clone {
    -webkit-box-decoration-break: clone;
    box-decoration-break: clone;
  }
  .box-decoration-slice {
    -webkit-box-decoration-break: slice;
    box-decoration-break: slice;
  }
  .bg-auto {
    background-size: auto;
  }
  .bg-contain {
    background-size: contain;
  }
  .bg-cover {
    background-size: cover;
  }
  .bg-fixed {
    background-attachment: fixed;
  }
  .bg-local {
    background-attachment: local;
  }
  .bg-scroll {
    background-attachment: scroll;
  }
  .bg-clip-border {
    background-clip: border-box;
  }
  .bg-clip-content {
    background-clip: content-box;
  }
  .bg-clip-padding {
    background-clip: padding-box;
  }
  .bg-clip-text {
    background-clip: text;
  }
  .bg-bottom {
    background-position: bottom;
  }
  .bg-bottom-left {
    background-position: left bottom;
  }
  .bg-bottom-right {
    background-position: right bottom;
  }
  .bg-center {
    background-position: center;
  }
  .bg-left {
    background-position: left;
  }
  .bg-right {
    background-position: right;
  }
  .bg-top {
    background-position: top;
  }
  .bg-top-left {
    background-position: left top;
  }
  .bg-top-right {
    background-position: right top;
  }
  .bg-no-repeat {
    background-repeat: no-repeat;
  }
  .bg-repeat {
    background-repeat: repeat;
  }
  .bg-repeat-round {
    background-repeat: round;
  }
  .bg-repeat-space {
    background-repeat: space;
  }
  .bg-repeat-x {
    background-repeat: repeat-x;
  }
  .bg-repeat-y {
    background-repeat: repeat-y;
  }
  .bg-origin-border {
    background-origin: border-box;
  }
  .bg-origin-content {
    background-origin: content-box;
  }
  .bg-origin-padding {
    background-origin: padding-box;
  }
  .mask-add {
    mask-composite: add;
  }
  .mask-exclude {
    mask-composite: exclude;
  }
  .mask-intersect {
    mask-composite: intersect;
  }
  .mask-subtract {
    mask-composite: subtract;
  }
  .mask-alpha {
    mask-mode: alpha;
  }
  .mask-luminance {
    mask-mode: luminance;
  }
  .mask-match {
    mask-mode: match-source;
  }
  .mask-type-alpha {
    mask-type: alpha;
  }
  .mask-type-luminance {
    mask-type: luminance;
  }
  .mask-auto {
    mask-size: auto;
  }
  .mask-contain {
    mask-size: contain;
  }
  .mask-cover {
    mask-size: cover;
  }
  .mask-clip-border {
    mask-clip: border-box;
  }
  .mask-clip-content {
    mask-clip: content-box;
  }
  .mask-clip-fill {
    mask-clip: fill-box;
  }
  .mask-clip-padding {
    mask-clip: padding-box;
  }
  .mask-clip-stroke {
    mask-clip: stroke-box;
  }
  .mask-clip-view {
    mask-clip: view-box;
  }
  .mask-no-clip {
    mask-clip: no-clip;
  }
  .mask-bottom {
    mask-position: bottom;
  }
  .mask-bottom-left {
    mask-position: left bottom;
  }
  .mask-bottom-right {
    mask-position: right bottom;
  }
  .mask-center {
    mask-position: center;
  }
  .mask-left {
    mask-position: left;
  }
  .mask-right {
    mask-position: right;
  }
  .mask-top {
    mask-position: top;
  }
  .mask-top-left {
    mask-position: left top;
  }
  .mask-top-right {
    mask-position: right top;
  }
  .mask-no-repeat {
    mask-repeat: no-repeat;
  }
  .mask-repeat {
    mask-repeat: repeat;
  }
  .mask-repeat-round {
    mask-repeat: round;
  }
  .mask-repeat-space {
    mask-repeat: space;
  }
  .mask-repeat-x {
    mask-repeat: repeat-x;
  }
  .mask-repeat-y {
    mask-repeat: repeat-y;
  }
  .mask-origin-border {
    mask-origin: border-box;
  }
  .mask-origin-content {
    mask-origin: content-box;
  }
  .mask-origin-fill {
    mask-origin: fill-box;
  }
  .mask-origin-padding {
    mask-origin: padding-box;
  }
  .mask-origin-stroke {
    mask-origin: stroke-box;
  }
  .mask-origin-view {
    mask-origin: view-box;
  }
  .fill-none {
    fill: none;
  }
  .stroke-none {
    stroke: none;
  }
  .object-contain {
    object-fit: contain;
  }
  .object-cover {
    object-fit: cover;
  }
  .object-fill {
    object-fit: fill;
  }
  .object-none {
    object-fit: none;
  }
  .object-scale-down {
    object-fit: scale-down;
  }
  .text-center {
    text-align: center;
  }
  .text-end {
    text-align: end;
  }
  .text-justify {
    text-align: justify;
  }
  .text-left {
    text-align: left;
  }
  .text-right {
    text-align: right;
  }
  .text-start {
    text-align: start;
  }
  .align-baseline {
    vertical-align: baseline;
  }
  .align-bottom {
    vertical-align: bottom;
  }
  .align-middle {
    vertical-align: middle;
  }
  .align-sub {
    vertical-align: sub;
  }
  .align-super {
    vertical-align: super;
  }
  .align-text-bottom {
    vertical-align: text-bottom;
  }
  .align-text-top {
    vertical-align: text-top;
  }
  .align-top {
    vertical-align: top;
  }
  .text-balance {
    text-wrap: balance;
  }
  .text-nowrap {
    text-wrap: nowrap;
  }
  .text-pretty {
    text-wrap: pretty;
  }
  .text-wrap {
    text-wrap: wrap;
  }
  .break-normal {
    overflow-wrap: normal;
    word-break: normal;
  }
  .wrap-anywhere {
    overflow-wrap: anywhere;
  }
  .wrap-break-word {
    overflow-wrap: break-word;
  }
  .wrap-normal {
    overflow-wrap: normal;
  }
  .break-all {
    word-break: break-all;
  }
  .break-keep {
    word-break: keep-all;
  }
  .text-clip {
    text-overflow: clip;
  }
  .text-ellipsis {
    text-overflow: ellipsis;
  }
  .hyphens-auto {
    -webkit-hyphens: auto;
    hyphens: auto;
  }
  .hyphens-manual {
    -webkit-hyphens: manual;
    hyphens: manual;
  }
  .hyphens-none {
    -webkit-hyphens: none;
    hyphens: none;
  }
  .whitespace-break-spaces {
    white-space: break-spaces;
  }
  .whitespace-normal {
    white-space: normal;
  }
  .whitespace-nowrap {
    white-space: nowrap;
  }
  .whitespace-pre {
    white-space: pre;
  }
  .whitespace-pre-line {
    white-space: pre-line;
  }
  .whitespace-pre-wrap {
    white-space: pre-wrap;
  }
  .capitalize {
    text-transform: capitalize;
  }
  .lowercase {
    text-transform: lowercase;
  }
  .normal-case {
    text-transform: none;
  }
  .uppercase {
    text-transform: uppercase;
  }
  .italic {
    font-style: italic;
  }
  .not-italic {
    font-style: normal;
  }
  .font-stretch-condensed {
    font-stretch: condensed;
  }
  .font-stretch-expanded {
    font-stretch: expanded;
  }
  .font-stretch-extra-condensed {
    font-stretch: extra-condensed;
  }
  .font-stretch-extra-expanded {
    font-stretch: extra-expanded;
  }
  .font-stretch-normal {
    font-stretch: normal;
  }
  .font-stretch-semi-condensed {
    font-stretch: semi-condensed;
  }
  .font-stretch-semi-expanded {
    font-stretch: semi-expanded;
  }
  .font-stretch-ultra-condensed {
    font-stretch: ultra-condensed;
  }
  .font-stretch-ultra-expanded {
    font-stretch: ultra-expanded;
  }
  .diagonal-fractions {
    --tw-numeric-fraction: diagonal-fractions;
    font-variant-numeric: var(--tw-ordinal,) var(--tw-slashed-zero,) var(--tw-numeric-figure,) var(--tw-numeric-spacing,) var(--tw-numeric-fraction,);
  }
  .lining-nums {
    --tw-numeric-figure: lining-nums;
    font-variant-numeric: var(--tw-ordinal,) var(--tw-slashed-zero,) var(--tw-numeric-figure,) var(--tw-numeric-spacing,) var(--tw-numeric-fraction,);
  }
  .oldstyle-nums {
    --tw-numeric-figure: oldstyle-nums;
    font-variant-numeric: var(--tw-ordinal,) var(--tw-slashed-zero,) var(--tw-numeric-figure,) var(--tw-numeric-spacing,) var(--tw-numeric-fraction,);
  }
  .ordinal {
    --tw-ordinal: ordinal;
    font-variant-numeric: var(--tw-ordinal,) var(--tw-slashed-zero,) var(--tw-numeric-figure,) var(--tw-numeric-spacing,) var(--tw-numeric-fraction,);
  }
  .proportional-nums {
    --tw-numeric-spacing: proportional-nums;
    font-variant-numeric: var(--tw-ordinal,) var(--tw-slashed-zero,) var(--tw-numeric-figure,) var(--tw-numeric-spacing,) var(--tw-numeric-fraction,);
  }
  .slashed-zero {
    --tw-slashed-zero: slashed-zero;
    font-variant-numeric: var(--tw-ordinal,) var(--tw-slashed-zero,) var(--tw-numeric-figure,) var(--tw-numeric-spacing,) var(--tw-numeric-fraction,);
  }
  .stacked-fractions {
    --tw-numeric-fraction: stacked-fractions;
    font-variant-numeric: var(--tw-ordinal,) var(--tw-slashed-zero,) var(--tw-numeric-figure,) var(--tw-numeric-spacing,) var(--tw-numeric-fraction,);
  }
  .tabular-nums {
    --tw-numeric-spacing: tabular-nums;
    font-variant-numeric: var(--tw-ordinal,) var(--tw-slashed-zero,) var(--tw-numeric-figure,) var(--tw-numeric-spacing,) var(--tw-numeric-fraction,);
  }
  .normal-nums {
    font-variant-numeric: normal;
  }
  .line-through {
    text-decoration-line: line-through;
  }
  .no-underline {
    text-decoration-line: none;
  }
  .overline {
    text-decoration-line: overline;
  }
  .underline {
    text-decoration-line: underline;
  }
  .decoration-dashed {
    text-decoration-style: dashed;
  }
  .decoration-dotted {
    text-decoration-style: dotted;
  }
  .decoration-double {
    text-decoration-style: double;
  }
  .decoration-solid {
    text-decoration-style: solid;
  }
  .decoration-wavy {
    text-decoration-style: wavy;
  }
  .decoration-auto {
    text-decoration-thickness: auto;
  }
  .decoration-from-font {
    text-decoration-thickness: from-font;
  }
  .antialiased {
    -webkit-font-smoothing: antialiased;
    -moz-osx-font-smoothing: grayscale;
  }
  .subpixel-antialiased {
    -webkit-font-smoothing: auto;
    -moz-osx-font-smoothing: auto;
  }
  .accent-auto {
    accent-color: auto;
  }
  .scheme-dark {
    color-scheme: dark;
  }
  .scheme-light {
    color-scheme: light;
  }
  .scheme-light-dark {
    color-scheme: light dark;
  }
  .scheme-normal {
    color-scheme: normal;
  }
  .scheme-only-dark {
    color-scheme: only dark;
  }
  .scheme-only-light {
    color-scheme: only light;
  }
  .mix-blend-plus-darker {
    mix-blend-mode: plus-darker;
  }
  .mix-blend-plus-lighter {
    mix-blend-mode: plus-lighter;
  }
  .shadow-initial {
    --tw-shadow-color: initial;
  }
  .inset-shadow-initial {
    --tw-inset-shadow-color: initial;
  }
  .drop-shadow-none {
    --tw-drop-shadow:  ;
    filter: var(--tw-blur,) var(--tw-brightness,) var(--tw-contrast,) var(--tw-grayscale,) var(--tw-hue-rotate,) var(--tw-invert,) var(--tw-saturate,) var(--tw-sepia,) var(--tw-drop-shadow,);
  }
  .transition-discrete {
    transition-behavior: allow-discrete;
  }
  .transition-normal {
    transition-behavior: normal;
  }
  .will-change-auto {
    will-change: auto;
  }
  .will-change-contents {
    will-change: contents;
  }
  .will-change-scroll {
    will-change: scroll-position;
  }
  .will-change-transform {
    will-change: transform;
  }
  .contain-inline-size {
    --tw-contain-size: inline-size;
    contain: var(--tw-contain-size,) var(--tw-contain-layout,) var(--tw-contain-paint,) var(--tw-contain-style,);
  }
  .contain-layout {
    --tw-contain-layout: layout;
    contain: var(--tw-contain-size,) var(--tw-contain-layout,) var(--tw-contain-paint,) var(--tw-contain-style,);
  }
  .contain-paint {
    --tw-contain-paint: paint;
    contain: var(--tw-contain-size,) var(--tw-contain-layout,) var(--tw-contain-paint,) var(--tw-contain-style,);
  }
  .contain-size {
    --tw-contain-size: size;
    contain: var(--tw-contain-size,) var(--tw-contain-layout,) var(--tw-contain-paint,) var(--tw-contain-style,);
  }
  .contain-style {
    --tw-contain-style: style;
    contain: var(--tw-contain-size,) var(--tw-contain-layout,) var(--tw-contain-paint,) var(--tw-contain-style,);
  }
  .contain-content {
    contain: content;
  }
  .contain-none {
    contain: none;
  }
  .contain-strict {
    contain: strict;
  }
  .content-none {
    --tw-content: none;
    content: none;
  }
  .forced-color-adjust-auto {
    forced-color-adjust: auto;
  }
  .forced-color-adjust-none {
    forced-color-adjust: none;
  }
  .outline-dashed {
    --tw-outline-style: dashed;
    outline-style: dashed;
  }
  .outline-dotted {
    --tw-outline-style: dotted;
    outline-style: dotted;
  }
  .outline-double {
    --tw-outline-style: double;
    outline-style: double;
  }
  .outline-none {
    --tw-outline-style: none;
    outline-style: none;
  }
  .outline-solid {
    --tw-outline-style: solid;
    outline-style: solid;
  }
  .backface-hidden {
    backface-visibility: hidden;
  }
  .backface-visible {
    backface-visibility: visible;
  }
  :where(.divide-x-reverse > :not(:last-child)) {
    --tw-divide-x-reverse: 1;
  }
  .duration-initial {
    --tw-duration: initial;
  }
  .ring-inset {
    --tw-ring-inset: inset;
  }
  .text-shadow-initial {
    --tw-text-shadow-color: initial;
  }
  .transform-3d {
    transform-style: preserve-3d;
  }
  .transform-border {
    transform-box: border-box;
  }
  .transform-content {
    transform-box: content-box;
  }
  .transform-fill {
    transform-box: fill-box;
  }
  .transform-flat {
    transform-style: flat;
  }
  .transform-stroke {
    transform-box: stroke-box;
  }
  .transform-view {
    transform-box: view-box;
  }
}
@property --tw-translate-x {
  syntax: "*";
  inherits: false;
  initial-value: 0;
}
@property --tw-translate-y {
  syntax: "*";
  inherits: false;
  initial-value: 0;
}
@property --tw-translate-z {
  syntax: "*";
  inherits: false;
  initial-value: 0;
}
@property --tw-scale-x {
  syntax: "*";
  inherits: false;
  initial-value: 1;
}
@property --tw-scale-y {
  syntax: "*";
  inherits: false;
  initial-value: 1;
}
@property --tw-scale-z {
  syntax: "*";
  inherits: false;
  initial-value: 1;
}
@property --tw-pan-x {
  syntax: "*";
  inherits: false;
}
@property --tw-pan-y {
  syntax: "*";
  inherits: false;
}
@property --tw-pinch-zoom {
  syntax: "*";
  inherits: false;
}
@property --tw-scroll-snap-strictness {
  syntax: "*";
  inherits: false;
  initial-value: proximity;
}
@property --tw-space-y-reverse {
  syntax: "*";
  inherits: false;
  initial-value: 0;
}
@property --tw-space-x-reverse {
  syntax: "*";
  inherits: false;
  initial-value: 0;
}
@property --tw-divide-y-reverse {
  syntax: "*";
  inherits: false;
  initial-value: 0;
}
@property --tw-ordinal {
  syntax: "*";
  inherits: false;
}
@property --tw-slashed-zero {
  syntax: "*";
  inherits: false;
}
@property --tw-numeric-figure {
  syntax: "*";
  inherits: false;
}
@property --tw-numeric-spacing {
  syntax: "*";
  inherits: false;
}
@property --tw-numeric-fraction {
  syntax: "*";
  inherits: false;
}
@property --tw-shadow {
  syntax: "*";
  inherits: false;
  initial-value: 0 0 #0000;
}
@property --tw-shadow-color {
  syntax: "*";
  inherits: false;
}
@property --tw-shadow-alpha {
  syntax: "<percentage>";
  inherits: false;
  initial-value: 100%;
}
@property --tw-inset-shadow {
  syntax: "*";
  inherits: false;
  initial-value: 0 0 #0000;
}
@property --tw-inset-shadow-color {
  syntax: "*";
  inherits: false;
}
@property --tw-inset-shadow-alpha {
  syntax: "<percentage>";
  inherits: false;
  initial-value: 100%;
}
@property --tw-ring-color {
  syntax: "*";
  inherits: false;
}
@property --tw-ring-shadow {
  syntax: "*";
  inherits: false;
  initial-value: 0 0 #0000;
}
@property --tw-inset-ring-color {
  syntax: "*";
  inherits: false;
}
@property --tw-inset-ring-shadow {
  syntax: "*";
  inherits: false;
  initial-value: 0 0 #0000;
}
@property --tw-ring-inset {
  syntax: "*";
  inherits: false;
}
@property --tw-ring-offset-width {
  syntax: "<length>";
  inherits: false;
  initial-value: 0px;
}
@property --tw-ring-offset-color {
  syntax: "*";
  inherits: false;
  initial-value: #fff;
}
@property --tw-ring-offset-shadow {
  syntax: "*";
  inherits: false;
  initial-value: 0 0 #0000;
}
@property --tw-blur {
  syntax: "*";
  inherits: false;
}
@property --tw-brightness {
  syntax: "*";
  inherits: false;
}
@property --tw-contrast {
  syntax: "*";
  inherits: false;
}
@property --tw-grayscale {
  syntax: "*";
  inherits: false;
}
@property --tw-hue-rotate {
  syntax: "*";
  inherits: false;
}
@property --tw-invert {
  syntax: "*";
  inherits: false;
}
@property --tw-opacity {
  syntax: "*";
  inherits: false;
}
@property --tw-saturate {
  syntax: "*";
  inherits: false;
}
@property --tw-sepia {
  syntax: "*";
  inherits: false;
}
@property --tw-drop-shadow {
  syntax: "*";
  inherits: false;
}
@property --tw-drop-shadow-color {
  syntax: "*";
  inherits: false;
}
@property --tw-drop-shadow-alpha {
  syntax: "<percentage>";
  inherits: false;
  initial-value: 100%;
}
@property --tw-drop-shadow-size {
  syntax: "*";
  inherits: false;
}
@property --tw-contain-size {
  syntax: "*";
  inherits: false;
}
@property --tw-contain-layout {
  syntax: "*";
  inherits: false;
}
@property --tw-contain-paint {
  syntax: "*";
  inherits: false;
}
@property --tw-contain-style {
  syntax: "*";
  inherits: false;
}
@property --tw-divide-x-reverse {
  syntax: "*";
  inherits: false;
  initial-value: 0;
}
@property --tw-duration {
  syntax: "*";
  inherits: false;
}
@property --tw-text-shadow-color {
  syntax: "*";
  inherits: false;
}
@property --tw-text-shadow-alpha {
  syntax: "<percentage>";
  inherits: false;
  initial-value: 100%;
}
@layer properties {
  @supports ((-webkit-hyphens: none) and (not (margin-trim: inline))) or ((-moz-orient: inline) and (not (color:rgb(from red r g b)))) {
    *, ::before, ::after, ::backdrop {
      --tw-translate-x: 0;
      --tw-translate-y: 0;
      --tw-translate-z: 0;
      --tw-scale-x: 1;
      --tw-scale-y: 1;
      --tw-scale-z: 1;
      --tw-pan-x: initial;
      --tw-pan-y: initial;
      --tw-pinch-zoom: initial;
      --tw-scroll-snap-strictness: proximity;
      --tw-space-y-reverse: 0;
      --tw-space-x-reverse: 0;
      --tw-divide-y-reverse: 0;
      --tw-ordinal: initial;
      --tw-slashed-zero: initial;
      --tw-numeric-figure: initial;
      --tw-numeric-spacing: initial;
      --tw-numeric-fraction: initial;
      --tw-shadow: 0 0 #0000;
      --tw-shadow-color: initial;
      --tw-shadow-alpha: 100%;
      --tw-inset-shadow: 0 0 #0000;
      --tw-inset-shadow-color: initial;
      --tw-inset-shadow-alpha: 100%;
      --tw-ring-color: initial;
      --tw-ring-shadow: 0 0 #0000;
      --tw-inset-ring-color: initial;
      --tw-inset-ring-shadow: 0 0 #0000;
      --tw-ring-inset: initial;
      --tw-ring-offset-width: 0px;
      --tw-ring-offset-color: #fff;
      --tw-ring-offset-shadow: 0 0 #0000;
      --tw-blur: initial;
      --tw-brightness: initial;
      --tw-contrast: initial;
      --tw-grayscale: initial;
      --tw-hue-rotate: initial;
      --tw-invert: initial;
      --tw-opacity: initial;
      --tw-saturate: initial;
      --tw-sepia: initial;
      --tw-drop-shadow: initial;
      --tw-drop-shadow-color: initial;
      --tw-drop-shadow-alpha: 100%;
      --tw-drop-shadow-size: initial;
      --tw-contain-size: initial;
      --tw-contain-layout: initial;
      --tw-contain-paint: initial;
      --tw-contain-style: initial;
      --tw-divide-x-reverse: 0;
      --tw-duration: initial;
      --tw-text-shadow-color: initial;
      --tw-text-shadow-alpha: 100%;
    }
  }
}
```

## `builtins.functional-utility-roots-with-arbitrary-values`

### Input CSS

```css
@import "tailwindcss/theme" layer(theme);
@import "tailwindcss/utilities" layer(utilities);
```

### Candidates

```text
align-[initial] animate-[initial] aspect-[initial] auto-cols-[initial] auto-rows-[initial] backdrop-blur-[initial] backdrop-brightness-[initial] backdrop-contrast-[initial] backdrop-grayscale-[initial] backdrop-hue-rotate-[initial]
backdrop-invert-[initial] backdrop-opacity-[initial] backdrop-saturate-[initial] backdrop-sepia-[initial] bg-position-[initial] bg-size-[initial] blur-[initial] brightness-[initial] col-[initial] col-end-[initial]
col-span-[initial] col-start-[initial] columns-[initial] contain-[initial] content-[initial] contrast-[initial] cursor-[initial] delay-[initial] divide-x-[initial] divide-y-[initial]
ease-[initial] font-features-[initial] font-stretch-[initial] grayscale-[initial] grid-cols-[initial] grid-rows-[initial] grow-[initial] hue-rotate-[initial] invert-[initial] line-clamp-[initial]
list-[initial] list-image-[initial] mask-conic-[initial] mask-linear-[initial] mask-position-[initial] mask-radial-[initial] mask-radial-at-[initial] mask-size-[initial] object-[initial] opacity-[initial]
order-[initial] origin-[initial] outline-offset-[initial] perspective-[initial] perspective-origin-[initial] row-[initial] row-end-[initial] row-span-[initial] row-start-[initial] saturate-[initial]
sepia-[initial] shrink-[initial] skew-[initial] skew-x-[initial] skew-y-[initial] tab-[initial] tracking-[initial] transition-[initial] underline-offset-[initial] will-change-[initial]
z-[initial] zoom-[initial]
```

### Expected CSS

```css
/*! tailwindcss v4.3.3 | MIT License | https://tailwindcss.com */
@layer properties;
@layer theme {
  :root, :host {
    --default-transition-duration: 150ms;
    --default-transition-timing-function: cubic-bezier(0.4, 0, 0.2, 1);
  }
}
@layer utilities {
  .z-\[initial\] {
    z-index: initial;
  }
  .order-\[initial\] {
    order: initial;
  }
  .col-\[initial\] {
    grid-column: initial;
  }
  .col-span-\[initial\] {
    grid-column: span initial / span initial;
  }
  .col-start-\[initial\] {
    grid-column-start: initial;
  }
  .col-end-\[initial\] {
    grid-column-end: initial;
  }
  .row-\[initial\] {
    grid-row: initial;
  }
  .row-span-\[initial\] {
    grid-row: span initial / span initial;
  }
  .row-start-\[initial\] {
    grid-row-start: initial;
  }
  .row-end-\[initial\] {
    grid-row-end: initial;
  }
  .line-clamp-\[initial\] {
    overflow: hidden;
    display: -webkit-box;
    -webkit-box-orient: vertical;
    -webkit-line-clamp: initial;
  }
  .aspect-\[initial\] {
    aspect-ratio: initial;
  }
  .shrink-\[initial\] {
    flex-shrink: initial;
  }
  .grow-\[initial\] {
    flex-grow: initial;
  }
  .origin-\[initial\] {
    transform-origin: initial;
  }
  .skew-\[initial\] {
    --tw-skew-x: skewX(initial);
    --tw-skew-y: skewY(initial);
    transform: var(--tw-rotate-x,) var(--tw-rotate-y,) var(--tw-rotate-z,) var(--tw-skew-x,) var(--tw-skew-y,);
  }
  .skew-x-\[initial\] {
    --tw-skew-x: skewX(initial);
    transform: var(--tw-rotate-x,) var(--tw-rotate-y,) var(--tw-rotate-z,) var(--tw-skew-x,) var(--tw-skew-y,);
  }
  .skew-y-\[initial\] {
    --tw-skew-y: skewY(initial);
    transform: var(--tw-rotate-x,) var(--tw-rotate-y,) var(--tw-rotate-z,) var(--tw-skew-x,) var(--tw-skew-y,);
  }
  .zoom-\[initial\] {
    zoom: initial;
  }
  .animate-\[initial\] {
    animation: initial;
  }
  .cursor-\[initial\] {
    cursor: initial;
  }
  .list-\[initial\] {
    list-style-type: initial;
  }
  .list-image-\[initial\] {
    list-style-image: initial;
  }
  .columns-\[initial\] {
    columns: initial;
  }
  .auto-cols-\[initial\] {
    grid-auto-columns: initial;
  }
  .auto-rows-\[initial\] {
    grid-auto-rows: initial;
  }
  .grid-cols-\[initial\] {
    grid-template-columns: initial;
  }
  .grid-rows-\[initial\] {
    grid-template-rows: initial;
  }
  :where(.divide-x-\[initial\] > :not(:last-child)) {
    --tw-divide-x-reverse: 0;
    border-inline-style: var(--tw-border-style);
    border-inline-start-width: calc(initial * var(--tw-divide-x-reverse));
    border-inline-end-width: calc(initial * calc(1 - var(--tw-divide-x-reverse)));
  }
  :where(.divide-y-\[initial\] > :not(:last-child)) {
    --tw-divide-y-reverse: 0;
    border-bottom-style: var(--tw-border-style);
    border-top-style: var(--tw-border-style);
    border-top-width: calc(initial * var(--tw-divide-y-reverse));
    border-bottom-width: calc(initial * calc(1 - var(--tw-divide-y-reverse)));
  }
  .mask-linear-\[initial\] {
    mask-image: var(--tw-mask-linear), var(--tw-mask-radial), var(--tw-mask-conic);
    mask-composite: intersect;
    --tw-mask-linear: linear-gradient(var(--tw-mask-linear-stops, var(--tw-mask-linear-position)));
    --tw-mask-linear-position: initial;
  }
  .mask-radial-\[initial\] {
    mask-image: var(--tw-mask-linear), var(--tw-mask-radial), var(--tw-mask-conic);
    mask-composite: intersect;
    --tw-mask-radial: radial-gradient(var(--tw-mask-radial-stops, var(--tw-mask-radial-size)));
    --tw-mask-radial-size: initial;
  }
  .mask-conic-\[initial\] {
    mask-image: var(--tw-mask-linear), var(--tw-mask-radial), var(--tw-mask-conic);
    mask-composite: intersect;
    --tw-mask-conic: conic-gradient(var(--tw-mask-conic-stops, var(--tw-mask-conic-position)));
    --tw-mask-conic-position: initial;
  }
  .mask-radial-at-\[initial\] {
    --tw-mask-radial-position: initial;
  }
  .bg-size-\[initial\] {
    background-size: initial;
  }
  .bg-position-\[initial\] {
    background-position: initial;
  }
  .mask-size-\[initial\] {
    mask-size: initial;
  }
  .mask-position-\[initial\] {
    mask-position: initial;
  }
  .object-\[initial\] {
    object-position: initial;
  }
  .align-\[initial\] {
    vertical-align: initial;
  }
  .font-features-\[initial\] {
    font-feature-settings: initial;
  }
  .tracking-\[initial\] {
    --tw-tracking: initial;
    letter-spacing: initial;
  }
  .tab-\[initial\] {
    tab-size: initial;
  }
  .font-stretch-\[initial\] {
    font-stretch: initial;
  }
  .underline-offset-\[initial\] {
    text-underline-offset: initial;
  }
  .opacity-\[initial\] {
    opacity: initial;
  }
  .outline-offset-\[initial\] {
    outline-offset: initial;
  }
  .blur-\[initial\] {
    --tw-blur: blur(initial);
    filter: var(--tw-blur,) var(--tw-brightness,) var(--tw-contrast,) var(--tw-grayscale,) var(--tw-hue-rotate,) var(--tw-invert,) var(--tw-saturate,) var(--tw-sepia,) var(--tw-drop-shadow,);
  }
  .brightness-\[initial\] {
    --tw-brightness: brightness(initial);
    filter: var(--tw-blur,) var(--tw-brightness,) var(--tw-contrast,) var(--tw-grayscale,) var(--tw-hue-rotate,) var(--tw-invert,) var(--tw-saturate,) var(--tw-sepia,) var(--tw-drop-shadow,);
  }
  .contrast-\[initial\] {
    --tw-contrast: contrast(initial);
    filter: var(--tw-blur,) var(--tw-brightness,) var(--tw-contrast,) var(--tw-grayscale,) var(--tw-hue-rotate,) var(--tw-invert,) var(--tw-saturate,) var(--tw-sepia,) var(--tw-drop-shadow,);
  }
  .grayscale-\[initial\] {
    --tw-grayscale: grayscale(initial);
    filter: var(--tw-blur,) var(--tw-brightness,) var(--tw-contrast,) var(--tw-grayscale,) var(--tw-hue-rotate,) var(--tw-invert,) var(--tw-saturate,) var(--tw-sepia,) var(--tw-drop-shadow,);
  }
  .hue-rotate-\[initial\] {
    --tw-hue-rotate: hue-rotate(initial);
    filter: var(--tw-blur,) var(--tw-brightness,) var(--tw-contrast,) var(--tw-grayscale,) var(--tw-hue-rotate,) var(--tw-invert,) var(--tw-saturate,) var(--tw-sepia,) var(--tw-drop-shadow,);
  }
  .invert-\[initial\] {
    --tw-invert: invert(initial);
    filter: var(--tw-blur,) var(--tw-brightness,) var(--tw-contrast,) var(--tw-grayscale,) var(--tw-hue-rotate,) var(--tw-invert,) var(--tw-saturate,) var(--tw-sepia,) var(--tw-drop-shadow,);
  }
  .saturate-\[initial\] {
    --tw-saturate: saturate(initial);
    filter: var(--tw-blur,) var(--tw-brightness,) var(--tw-contrast,) var(--tw-grayscale,) var(--tw-hue-rotate,) var(--tw-invert,) var(--tw-saturate,) var(--tw-sepia,) var(--tw-drop-shadow,);
  }
  .sepia-\[initial\] {
    --tw-sepia: sepia(initial);
    filter: var(--tw-blur,) var(--tw-brightness,) var(--tw-contrast,) var(--tw-grayscale,) var(--tw-hue-rotate,) var(--tw-invert,) var(--tw-saturate,) var(--tw-sepia,) var(--tw-drop-shadow,);
  }
  .backdrop-blur-\[initial\] {
    --tw-backdrop-blur: blur(initial);
    -webkit-backdrop-filter: var(--tw-backdrop-blur,) var(--tw-backdrop-brightness,) var(--tw-backdrop-contrast,) var(--tw-backdrop-grayscale,) var(--tw-backdrop-hue-rotate,) var(--tw-backdrop-invert,) var(--tw-backdrop-opacity,) var(--tw-backdrop-saturate,) var(--tw-backdrop-sepia,);
    backdrop-filter: var(--tw-backdrop-blur,) var(--tw-backdrop-brightness,) var(--tw-backdrop-contrast,) var(--tw-backdrop-grayscale,) var(--tw-backdrop-hue-rotate,) var(--tw-backdrop-invert,) var(--tw-backdrop-opacity,) var(--tw-backdrop-saturate,) var(--tw-backdrop-sepia,);
  }
  .backdrop-brightness-\[initial\] {
    --tw-backdrop-brightness: brightness(initial);
    -webkit-backdrop-filter: var(--tw-backdrop-blur,) var(--tw-backdrop-brightness,) var(--tw-backdrop-contrast,) var(--tw-backdrop-grayscale,) var(--tw-backdrop-hue-rotate,) var(--tw-backdrop-invert,) var(--tw-backdrop-opacity,) var(--tw-backdrop-saturate,) var(--tw-backdrop-sepia,);
    backdrop-filter: var(--tw-backdrop-blur,) var(--tw-backdrop-brightness,) var(--tw-backdrop-contrast,) var(--tw-backdrop-grayscale,) var(--tw-backdrop-hue-rotate,) var(--tw-backdrop-invert,) var(--tw-backdrop-opacity,) var(--tw-backdrop-saturate,) var(--tw-backdrop-sepia,);
  }
  .backdrop-contrast-\[initial\] {
    --tw-backdrop-contrast: contrast(initial);
    -webkit-backdrop-filter: var(--tw-backdrop-blur,) var(--tw-backdrop-brightness,) var(--tw-backdrop-contrast,) var(--tw-backdrop-grayscale,) var(--tw-backdrop-hue-rotate,) var(--tw-backdrop-invert,) var(--tw-backdrop-opacity,) var(--tw-backdrop-saturate,) var(--tw-backdrop-sepia,);
    backdrop-filter: var(--tw-backdrop-blur,) var(--tw-backdrop-brightness,) var(--tw-backdrop-contrast,) var(--tw-backdrop-grayscale,) var(--tw-backdrop-hue-rotate,) var(--tw-backdrop-invert,) var(--tw-backdrop-opacity,) var(--tw-backdrop-saturate,) var(--tw-backdrop-sepia,);
  }
  .backdrop-grayscale-\[initial\] {
    --tw-backdrop-grayscale: grayscale(initial);
    -webkit-backdrop-filter: var(--tw-backdrop-blur,) var(--tw-backdrop-brightness,) var(--tw-backdrop-contrast,) var(--tw-backdrop-grayscale,) var(--tw-backdrop-hue-rotate,) var(--tw-backdrop-invert,) var(--tw-backdrop-opacity,) var(--tw-backdrop-saturate,) var(--tw-backdrop-sepia,);
    backdrop-filter: var(--tw-backdrop-blur,) var(--tw-backdrop-brightness,) var(--tw-backdrop-contrast,) var(--tw-backdrop-grayscale,) var(--tw-backdrop-hue-rotate,) var(--tw-backdrop-invert,) var(--tw-backdrop-opacity,) var(--tw-backdrop-saturate,) var(--tw-backdrop-sepia,);
  }
  .backdrop-hue-rotate-\[initial\] {
    --tw-backdrop-hue-rotate: hue-rotate(initial);
    -webkit-backdrop-filter: var(--tw-backdrop-blur,) var(--tw-backdrop-brightness,) var(--tw-backdrop-contrast,) var(--tw-backdrop-grayscale,) var(--tw-backdrop-hue-rotate,) var(--tw-backdrop-invert,) var(--tw-backdrop-opacity,) var(--tw-backdrop-saturate,) var(--tw-backdrop-sepia,);
    backdrop-filter: var(--tw-backdrop-blur,) var(--tw-backdrop-brightness,) var(--tw-backdrop-contrast,) var(--tw-backdrop-grayscale,) var(--tw-backdrop-hue-rotate,) var(--tw-backdrop-invert,) var(--tw-backdrop-opacity,) var(--tw-backdrop-saturate,) var(--tw-backdrop-sepia,);
  }
  .backdrop-invert-\[initial\] {
    --tw-backdrop-invert: invert(initial);
    -webkit-backdrop-filter: var(--tw-backdrop-blur,) var(--tw-backdrop-brightness,) var(--tw-backdrop-contrast,) var(--tw-backdrop-grayscale,) var(--tw-backdrop-hue-rotate,) var(--tw-backdrop-invert,) var(--tw-backdrop-opacity,) var(--tw-backdrop-saturate,) var(--tw-backdrop-sepia,);
    backdrop-filter: var(--tw-backdrop-blur,) var(--tw-backdrop-brightness,) var(--tw-backdrop-contrast,) var(--tw-backdrop-grayscale,) var(--tw-backdrop-hue-rotate,) var(--tw-backdrop-invert,) var(--tw-backdrop-opacity,) var(--tw-backdrop-saturate,) var(--tw-backdrop-sepia,);
  }
  .backdrop-opacity-\[initial\] {
    --tw-backdrop-opacity: opacity(initial);
    -webkit-backdrop-filter: var(--tw-backdrop-blur,) var(--tw-backdrop-brightness,) var(--tw-backdrop-contrast,) var(--tw-backdrop-grayscale,) var(--tw-backdrop-hue-rotate,) var(--tw-backdrop-invert,) var(--tw-backdrop-opacity,) var(--tw-backdrop-saturate,) var(--tw-backdrop-sepia,);
    backdrop-filter: var(--tw-backdrop-blur,) var(--tw-backdrop-brightness,) var(--tw-backdrop-contrast,) var(--tw-backdrop-grayscale,) var(--tw-backdrop-hue-rotate,) var(--tw-backdrop-invert,) var(--tw-backdrop-opacity,) var(--tw-backdrop-saturate,) var(--tw-backdrop-sepia,);
  }
  .backdrop-saturate-\[initial\] {
    --tw-backdrop-saturate: saturate(initial);
    -webkit-backdrop-filter: var(--tw-backdrop-blur,) var(--tw-backdrop-brightness,) var(--tw-backdrop-contrast,) var(--tw-backdrop-grayscale,) var(--tw-backdrop-hue-rotate,) var(--tw-backdrop-invert,) var(--tw-backdrop-opacity,) var(--tw-backdrop-saturate,) var(--tw-backdrop-sepia,);
    backdrop-filter: var(--tw-backdrop-blur,) var(--tw-backdrop-brightness,) var(--tw-backdrop-contrast,) var(--tw-backdrop-grayscale,) var(--tw-backdrop-hue-rotate,) var(--tw-backdrop-invert,) var(--tw-backdrop-opacity,) var(--tw-backdrop-saturate,) var(--tw-backdrop-sepia,);
  }
  .backdrop-sepia-\[initial\] {
    --tw-backdrop-sepia: sepia(initial);
    -webkit-backdrop-filter: var(--tw-backdrop-blur,) var(--tw-backdrop-brightness,) var(--tw-backdrop-contrast,) var(--tw-backdrop-grayscale,) var(--tw-backdrop-hue-rotate,) var(--tw-backdrop-invert,) var(--tw-backdrop-opacity,) var(--tw-backdrop-saturate,) var(--tw-backdrop-sepia,);
    backdrop-filter: var(--tw-backdrop-blur,) var(--tw-backdrop-brightness,) var(--tw-backdrop-contrast,) var(--tw-backdrop-grayscale,) var(--tw-backdrop-hue-rotate,) var(--tw-backdrop-invert,) var(--tw-backdrop-opacity,) var(--tw-backdrop-saturate,) var(--tw-backdrop-sepia,);
  }
  .transition-\[initial\] {
    transition-property: initial;
    transition-timing-function: var(--tw-ease, var(--default-transition-timing-function));
    transition-duration: var(--tw-duration, var(--default-transition-duration));
  }
  .delay-\[initial\] {
    transition-delay: initial;
  }
  .ease-\[initial\] {
    --tw-ease: initial;
    transition-timing-function: initial;
  }
  .will-change-\[initial\] {
    will-change: initial;
  }
  .contain-\[initial\] {
    contain: initial;
  }
  .content-\[initial\] {
    --tw-content: initial;
    content: var(--tw-content);
  }
  .perspective-\[initial\] {
    perspective: initial;
  }
  .perspective-origin-\[initial\] {
    perspective-origin: initial;
  }
}
@property --tw-rotate-x {
  syntax: "*";
  inherits: false;
}
@property --tw-rotate-y {
  syntax: "*";
  inherits: false;
}
@property --tw-rotate-z {
  syntax: "*";
  inherits: false;
}
@property --tw-skew-x {
  syntax: "*";
  inherits: false;
}
@property --tw-skew-y {
  syntax: "*";
  inherits: false;
}
@property --tw-divide-x-reverse {
  syntax: "*";
  inherits: false;
  initial-value: 0;
}
@property --tw-border-style {
  syntax: "*";
  inherits: false;
  initial-value: solid;
}
@property --tw-divide-y-reverse {
  syntax: "*";
  inherits: false;
  initial-value: 0;
}
@property --tw-mask-linear {
  syntax: "*";
  inherits: false;
  initial-value: linear-gradient(#fff, #fff);
}
@property --tw-mask-radial {
  syntax: "*";
  inherits: false;
  initial-value: linear-gradient(#fff, #fff);
}
@property --tw-mask-conic {
  syntax: "*";
  inherits: false;
  initial-value: linear-gradient(#fff, #fff);
}
@property --tw-mask-linear-position {
  syntax: "*";
  inherits: false;
  initial-value: 0deg;
}
@property --tw-mask-linear-from-position {
  syntax: "*";
  inherits: false;
  initial-value: 0%;
}
@property --tw-mask-linear-to-position {
  syntax: "*";
  inherits: false;
  initial-value: 100%;
}
@property --tw-mask-linear-from-color {
  syntax: "*";
  inherits: false;
  initial-value: black;
}
@property --tw-mask-linear-to-color {
  syntax: "*";
  inherits: false;
  initial-value: transparent;
}
@property --tw-mask-radial-from-position {
  syntax: "*";
  inherits: false;
  initial-value: 0%;
}
@property --tw-mask-radial-to-position {
  syntax: "*";
  inherits: false;
  initial-value: 100%;
}
@property --tw-mask-radial-from-color {
  syntax: "*";
  inherits: false;
  initial-value: black;
}
@property --tw-mask-radial-to-color {
  syntax: "*";
  inherits: false;
  initial-value: transparent;
}
@property --tw-mask-radial-shape {
  syntax: "*";
  inherits: false;
  initial-value: ellipse;
}
@property --tw-mask-radial-size {
  syntax: "*";
  inherits: false;
  initial-value: farthest-corner;
}
@property --tw-mask-radial-position {
  syntax: "*";
  inherits: false;
  initial-value: center;
}
@property --tw-mask-conic-position {
  syntax: "*";
  inherits: false;
  initial-value: 0deg;
}
@property --tw-mask-conic-from-position {
  syntax: "*";
  inherits: false;
  initial-value: 0%;
}
@property --tw-mask-conic-to-position {
  syntax: "*";
  inherits: false;
  initial-value: 100%;
}
@property --tw-mask-conic-from-color {
  syntax: "*";
  inherits: false;
  initial-value: black;
}
@property --tw-mask-conic-to-color {
  syntax: "*";
  inherits: false;
  initial-value: transparent;
}
@property --tw-tracking {
  syntax: "*";
  inherits: false;
}
@property --tw-blur {
  syntax: "*";
  inherits: false;
}
@property --tw-brightness {
  syntax: "*";
  inherits: false;
}
@property --tw-contrast {
  syntax: "*";
  inherits: false;
}
@property --tw-grayscale {
  syntax: "*";
  inherits: false;
}
@property --tw-hue-rotate {
  syntax: "*";
  inherits: false;
}
@property --tw-invert {
  syntax: "*";
  inherits: false;
}
@property --tw-opacity {
  syntax: "*";
  inherits: false;
}
@property --tw-saturate {
  syntax: "*";
  inherits: false;
}
@property --tw-sepia {
  syntax: "*";
  inherits: false;
}
@property --tw-drop-shadow {
  syntax: "*";
  inherits: false;
}
@property --tw-drop-shadow-color {
  syntax: "*";
  inherits: false;
}
@property --tw-drop-shadow-alpha {
  syntax: "<percentage>";
  inherits: false;
  initial-value: 100%;
}
@property --tw-drop-shadow-size {
  syntax: "*";
  inherits: false;
}
@property --tw-backdrop-blur {
  syntax: "*";
  inherits: false;
}
@property --tw-backdrop-brightness {
  syntax: "*";
  inherits: false;
}
@property --tw-backdrop-contrast {
  syntax: "*";
  inherits: false;
}
@property --tw-backdrop-grayscale {
  syntax: "*";
  inherits: false;
}
@property --tw-backdrop-hue-rotate {
  syntax: "*";
  inherits: false;
}
@property --tw-backdrop-invert {
  syntax: "*";
  inherits: false;
}
@property --tw-backdrop-opacity {
  syntax: "*";
  inherits: false;
}
@property --tw-backdrop-saturate {
  syntax: "*";
  inherits: false;
}
@property --tw-backdrop-sepia {
  syntax: "*";
  inherits: false;
}
@property --tw-ease {
  syntax: "*";
  inherits: false;
}
@property --tw-content {
  syntax: "*";
  inherits: false;
  initial-value: "";
}
@layer properties {
  @supports ((-webkit-hyphens: none) and (not (margin-trim: inline))) or ((-moz-orient: inline) and (not (color:rgb(from red r g b)))) {
    *, ::before, ::after, ::backdrop {
      --tw-rotate-x: initial;
      --tw-rotate-y: initial;
      --tw-rotate-z: initial;
      --tw-skew-x: initial;
      --tw-skew-y: initial;
      --tw-divide-x-reverse: 0;
      --tw-border-style: solid;
      --tw-divide-y-reverse: 0;
      --tw-mask-linear: linear-gradient(#fff, #fff);
      --tw-mask-radial: linear-gradient(#fff, #fff);
      --tw-mask-conic: linear-gradient(#fff, #fff);
      --tw-mask-linear-position: 0deg;
      --tw-mask-linear-from-position: 0%;
      --tw-mask-linear-to-position: 100%;
      --tw-mask-linear-from-color: black;
      --tw-mask-linear-to-color: transparent;
      --tw-mask-radial-from-position: 0%;
      --tw-mask-radial-to-position: 100%;
      --tw-mask-radial-from-color: black;
      --tw-mask-radial-to-color: transparent;
      --tw-mask-radial-shape: ellipse;
      --tw-mask-radial-size: farthest-corner;
      --tw-mask-radial-position: center;
      --tw-mask-conic-position: 0deg;
      --tw-mask-conic-from-position: 0%;
      --tw-mask-conic-to-position: 100%;
      --tw-mask-conic-from-color: black;
      --tw-mask-conic-to-color: transparent;
      --tw-tracking: initial;
      --tw-blur: initial;
      --tw-brightness: initial;
      --tw-contrast: initial;
      --tw-grayscale: initial;
      --tw-hue-rotate: initial;
      --tw-invert: initial;
      --tw-opacity: initial;
      --tw-saturate: initial;
      --tw-sepia: initial;
      --tw-drop-shadow: initial;
      --tw-drop-shadow-color: initial;
      --tw-drop-shadow-alpha: 100%;
      --tw-drop-shadow-size: initial;
      --tw-backdrop-blur: initial;
      --tw-backdrop-brightness: initial;
      --tw-backdrop-contrast: initial;
      --tw-backdrop-grayscale: initial;
      --tw-backdrop-hue-rotate: initial;
      --tw-backdrop-invert: initial;
      --tw-backdrop-opacity: initial;
      --tw-backdrop-saturate: initial;
      --tw-backdrop-sepia: initial;
      --tw-ease: initial;
      --tw-content: "";
    }
  }
}
```

## `builtins.static-variants`

### Input CSS

```css
@import "tailwindcss/theme" layer(theme);
@import "tailwindcss/utilities" layer(utilities);
```

### Candidates

```text
*:bg-red-500 **:bg-red-500 active:bg-red-500 after:bg-red-500 any-pointer-coarse:bg-red-500 any-pointer-fine:bg-red-500 any-pointer-none:bg-red-500 autofill:bg-red-500 backdrop:bg-red-500 before:bg-red-500
checked:bg-red-500 contrast-less:bg-red-500 contrast-more:bg-red-500 dark:bg-red-500 default:bg-red-500 details-content:bg-red-500 disabled:bg-red-500 empty:bg-red-500 enabled:bg-red-500 even:bg-red-500
file:bg-red-500 first:bg-red-500 first-letter:bg-red-500 first-line:bg-red-500 first-of-type:bg-red-500 focus:bg-red-500 focus-visible:bg-red-500 focus-within:bg-red-500 forced-colors:bg-red-500 hover:bg-red-500
in-range:bg-red-500 indeterminate:bg-red-500 inert:bg-red-500 invalid:bg-red-500 inverted-colors:bg-red-500 landscape:bg-red-500 last:bg-red-500 last-of-type:bg-red-500 ltr:bg-red-500 marker:bg-red-500
motion-reduce:bg-red-500 motion-safe:bg-red-500 noscript:bg-red-500 odd:bg-red-500 only:bg-red-500 only-of-type:bg-red-500 open:bg-red-500 optional:bg-red-500 out-of-range:bg-red-500 placeholder:bg-red-500
placeholder-shown:bg-red-500 pointer-coarse:bg-red-500 pointer-fine:bg-red-500 pointer-none:bg-red-500 portrait:bg-red-500 print:bg-red-500 read-only:bg-red-500 required:bg-red-500 rtl:bg-red-500 selection:bg-red-500
starting:bg-red-500 target:bg-red-500 user-invalid:bg-red-500 user-valid:bg-red-500 valid:bg-red-500 visited:bg-red-500
```

### Expected CSS

```css
/*! tailwindcss v4.3.3 | MIT License | https://tailwindcss.com */
@layer properties;
@layer theme {
  :root, :host {
    --color-red-500: oklch(63.7% 0.237 25.331);
  }
}
@layer utilities {
  :is(.\*\:bg-red-500 > *) {
    background-color: var(--color-red-500);
  }
  :is(.\*\*\:bg-red-500 *) {
    background-color: var(--color-red-500);
  }
  .first-letter\:bg-red-500::first-letter {
    background-color: var(--color-red-500);
  }
  .first-line\:bg-red-500::first-line {
    background-color: var(--color-red-500);
  }
  .marker\:bg-red-500 ::marker {
    background-color: var(--color-red-500);
  }
  .marker\:bg-red-500::marker {
    background-color: var(--color-red-500);
  }
  .marker\:bg-red-500 ::-webkit-details-marker {
    background-color: var(--color-red-500);
  }
  .marker\:bg-red-500::-webkit-details-marker {
    background-color: var(--color-red-500);
  }
  .selection\:bg-red-500 ::selection {
    background-color: var(--color-red-500);
  }
  .selection\:bg-red-500::selection {
    background-color: var(--color-red-500);
  }
  .file\:bg-red-500::file-selector-button {
    background-color: var(--color-red-500);
  }
  .placeholder\:bg-red-500::placeholder {
    background-color: var(--color-red-500);
  }
  .backdrop\:bg-red-500::backdrop {
    background-color: var(--color-red-500);
  }
  .details-content\:bg-red-500::details-content {
    background-color: var(--color-red-500);
  }
  .before\:bg-red-500::before {
    content: var(--tw-content);
    background-color: var(--color-red-500);
  }
  .after\:bg-red-500::after {
    content: var(--tw-content);
    background-color: var(--color-red-500);
  }
  .first\:bg-red-500:first-child {
    background-color: var(--color-red-500);
  }
  .last\:bg-red-500:last-child {
    background-color: var(--color-red-500);
  }
  .only\:bg-red-500:only-child {
    background-color: var(--color-red-500);
  }
  .odd\:bg-red-500:nth-child(odd) {
    background-color: var(--color-red-500);
  }
  .even\:bg-red-500:nth-child(even) {
    background-color: var(--color-red-500);
  }
  .first-of-type\:bg-red-500:first-of-type {
    background-color: var(--color-red-500);
  }
  .last-of-type\:bg-red-500:last-of-type {
    background-color: var(--color-red-500);
  }
  .only-of-type\:bg-red-500:only-of-type {
    background-color: var(--color-red-500);
  }
  .visited\:bg-red-500:visited {
    background-color: var(--color-red-500);
  }
  .target\:bg-red-500:target {
    background-color: var(--color-red-500);
  }
  .open\:bg-red-500:is([open], :popover-open, :open) {
    background-color: var(--color-red-500);
  }
  .default\:bg-red-500:default {
    background-color: var(--color-red-500);
  }
  .checked\:bg-red-500:checked {
    background-color: var(--color-red-500);
  }
  .indeterminate\:bg-red-500:indeterminate {
    background-color: var(--color-red-500);
  }
  .placeholder-shown\:bg-red-500:placeholder-shown {
    background-color: var(--color-red-500);
  }
  .autofill\:bg-red-500:autofill {
    background-color: var(--color-red-500);
  }
  .optional\:bg-red-500:optional {
    background-color: var(--color-red-500);
  }
  .required\:bg-red-500:required {
    background-color: var(--color-red-500);
  }
  .valid\:bg-red-500:valid {
    background-color: var(--color-red-500);
  }
  .invalid\:bg-red-500:invalid {
    background-color: var(--color-red-500);
  }
  .user-valid\:bg-red-500:user-valid {
    background-color: var(--color-red-500);
  }
  .user-invalid\:bg-red-500:user-invalid {
    background-color: var(--color-red-500);
  }
  .in-range\:bg-red-500:in-range {
    background-color: var(--color-red-500);
  }
  .out-of-range\:bg-red-500:out-of-range {
    background-color: var(--color-red-500);
  }
  .read-only\:bg-red-500:read-only {
    background-color: var(--color-red-500);
  }
  .empty\:bg-red-500:empty {
    background-color: var(--color-red-500);
  }
  .focus-within\:bg-red-500:focus-within {
    background-color: var(--color-red-500);
  }
  @media (hover: hover) {
    .hover\:bg-red-500:hover {
      background-color: var(--color-red-500);
    }
  }
  .focus\:bg-red-500:focus {
    background-color: var(--color-red-500);
  }
  .focus-visible\:bg-red-500:focus-visible {
    background-color: var(--color-red-500);
  }
  .active\:bg-red-500:active {
    background-color: var(--color-red-500);
  }
  .enabled\:bg-red-500:enabled {
    background-color: var(--color-red-500);
  }
  .disabled\:bg-red-500:disabled {
    background-color: var(--color-red-500);
  }
  .inert\:bg-red-500:is([inert], [inert] *) {
    background-color: var(--color-red-500);
  }
  @media (prefers-reduced-motion: no-preference) {
    .motion-safe\:bg-red-500 {
      background-color: var(--color-red-500);
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .motion-reduce\:bg-red-500 {
      background-color: var(--color-red-500);
    }
  }
  @media (prefers-contrast: more) {
    .contrast-more\:bg-red-500 {
      background-color: var(--color-red-500);
    }
  }
  @media (prefers-contrast: less) {
    .contrast-less\:bg-red-500 {
      background-color: var(--color-red-500);
    }
  }
  @media (orientation: portrait) {
    .portrait\:bg-red-500 {
      background-color: var(--color-red-500);
    }
  }
  @media (orientation: landscape) {
    .landscape\:bg-red-500 {
      background-color: var(--color-red-500);
    }
  }
  .ltr\:bg-red-500:where(:dir(ltr), [dir="ltr"], [dir="ltr"] *) {
    background-color: var(--color-red-500);
  }
  .rtl\:bg-red-500:where(:dir(rtl), [dir="rtl"], [dir="rtl"] *) {
    background-color: var(--color-red-500);
  }
  @media (prefers-color-scheme: dark) {
    .dark\:bg-red-500 {
      background-color: var(--color-red-500);
    }
  }
  @starting-style {
    .starting\:bg-red-500 {
      background-color: var(--color-red-500);
    }
  }
  @media print {
    .print\:bg-red-500 {
      background-color: var(--color-red-500);
    }
  }
  @media (forced-colors: active) {
    .forced-colors\:bg-red-500 {
      background-color: var(--color-red-500);
    }
  }
  @media (inverted-colors: inverted) {
    .inverted-colors\:bg-red-500 {
      background-color: var(--color-red-500);
    }
  }
  @media (pointer: none) {
    .pointer-none\:bg-red-500 {
      background-color: var(--color-red-500);
    }
  }
  @media (pointer: coarse) {
    .pointer-coarse\:bg-red-500 {
      background-color: var(--color-red-500);
    }
  }
  @media (pointer: fine) {
    .pointer-fine\:bg-red-500 {
      background-color: var(--color-red-500);
    }
  }
  @media (any-pointer: none) {
    .any-pointer-none\:bg-red-500 {
      background-color: var(--color-red-500);
    }
  }
  @media (any-pointer: coarse) {
    .any-pointer-coarse\:bg-red-500 {
      background-color: var(--color-red-500);
    }
  }
  @media (any-pointer: fine) {
    .any-pointer-fine\:bg-red-500 {
      background-color: var(--color-red-500);
    }
  }
  @media (scripting: none) {
    .noscript\:bg-red-500 {
      background-color: var(--color-red-500);
    }
  }
}
@property --tw-content {
  syntax: "*";
  initial-value: "";
  inherits: false;
}
@layer properties {
  @supports ((-webkit-hyphens: none) and (not (margin-trim: inline))) or ((-moz-orient: inline) and (not (color:rgb(from red r g b)))) {
    *, ::before, ::after, ::backdrop {
      --tw-content: "";
    }
  }
}
```
