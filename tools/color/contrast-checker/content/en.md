## About

Contrast uses relative sRGB luminance: (lighter L + 0.05) / (darker L + 0.05). Decisions use the unrounded ratio. This checks one pair of opaque colors, not the accessibility of an entire page.

## How to use this tool

1. Choose text and background colors.
2. Run the measurement.
3. Review normal-text and large-text thresholds separately.

## Frequently asked questions

### What are AA thresholds?

4.5:1 for normal text and 3:1 for large text under WCAG 1.4.3.

### What counts as large text?

At least 18 points, or 14 points bold, under the WCAG definition. Check actual size and weight in your interface.

### Are background images or transparency included?

No. Use the actual composited colors shown on screen; alpha colors are rejected.

Reference: [W3C WCAG, Contrast Minimum](https://www.w3.org/WAI/WCAG21/Understanding/contrast-minimum).
