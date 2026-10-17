## About

Enter an opaque short or long HEX color, RGB 0–255, HSL or OKLCH. Conversions use CSS Color 4 color spaces. Out-of-sRGB OKLCH colors are flagged and their outputs use channel clipping.

## How to use this tool

1. Choose an input format.
2. Enter #f00, rgb(255 0 0), hsl(0 100% 50%) or oklch(0.627955 0.257683 29.23).
3. Copy an output and check the gamut indicator.

## Frequently asked questions

### Is transparency supported?

No. This tool handles opaque colors and rejects alpha syntax.

### What does outside gamut mean?

The color exceeds sRGB. RGB channels are clipped, which can change the hue; the result is flagged.

### Which formats can I enter?

3- or 6-digit HEX, numeric RGB 0–255, HSL in degrees and percentages, or OKLCH with L from 0 to 1 or a percentage.

Reference: [W3C CSS Color 4](https://www.w3.org/TR/css-color/).
