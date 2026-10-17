# Worker smoke test

Build the engines first:

```sh
cargo xtask build secure.hash-calculator
cargo xtask build pdf.merge
```

Run the grouped Chromium recipe with Playwright available in the test environment:

```sh
node scripts/tests/worker-smoke.mjs
```

If Playwright is supplied externally, set `NODE_PATH` to its containing package directory.
Set `CHROME_PATH` to the Chrome executable on Linux/Windows; macOS defaults to the installed Google Chrome application. The test starts an ephemeral loopback server and a fresh browser context, without accessing personal browser data or a deployed service.

The assertions cover known SHA-256 vectors, a 64 MiB binary input against Node's crypto implementation, transferred ownership, a live UI event loop, valid PDF output, corrupt input, cancellation, timeout, recovery, loading failure, and offline hash execution/reload. They exercise the production JS bridge and compiled wasm engines. They do not replace the file-picker UI recipe, validate maximum contract sizes, or measure total browser/wasm memory.

Run once after changes to the Worker protocol or offline build pipeline; avoid repeating it for unrelated text/calculation tools. Tool-engine tests remain grouped by catalogue batch.
