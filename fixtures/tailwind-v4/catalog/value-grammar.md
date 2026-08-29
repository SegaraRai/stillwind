# Value grammar acceptance

This compact catalog fixes the Tailwind default-theme acceptance boundary for
representative unbracketed numeric utility families. It is executable against
the Tailwind compiler by `tools/verify-tailwind-v4-value-grammar.ts`; the
Stillwind resolver consumes the same expectations in its unit tests.

Arbitrary values are intentionally absent. Tailwind forwards their payloads
without property-level validation, whereas Stillwind validates them against the
CSS value type declared by the utility.

## Padding: accepted without brackets

```text
p-px p-0 p-0.5 p-1 p-1.5 p-4 p-50
```

## Padding: rejected without brackets

```text
p-50% p-auto p-red
```

## Border width: accepted without brackets

```text
border border-0 border-1 border-2 border-4 border-8 border-50
```

## Border width: rejected without brackets

```text
border-px border-red
```

## Z index: accepted without brackets

```text
z-auto z-0 z-1 z-4 z-50
```

## Z index: rejected without brackets

```text
z-px z-1.5 z-red
```
