# Self-hosted fonts

- `inter-latin-wght-normal.woff2`: Inter Variable, latin subset (covers French and English), from
  `@fontsource-variable/inter` 5.3.0 (npm tarball sha512 `OupL48va…b6b1oUA==` verified against the
  registry before extraction). Licence: SIL Open Font License 1.1, see `Inter-OFL-1.1.txt`.
- `cargo xtask tokens` embeds the file as a `data:` URI `@font-face` at the top of
  `design/dist/kortex.css`, so the same stylesheet works on the site, in tool iframes, in the apps
  and in desktop webviews without any host serving `/fonts`.
- Not self-hosted yet: the monospace face (`--k-type-mono` falls back to system monospace).
