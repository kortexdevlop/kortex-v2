# Changelog

## 1.0.0
- Initial local web and desktop engine: JPEG quality, PNG palette reduction, lossless WebP and optional downscaling.

## 1.0.2
- Always re-encode sanitized image output, including when the result grows, so source metadata is never returned as a fallback.
