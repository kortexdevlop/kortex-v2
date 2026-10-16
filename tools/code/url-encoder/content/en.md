## Encode and decode URLs
Turn text into a URL-safe version (spaces to %20, accents to %C3%A9…) or decode an encoded URL. Behaviour follows JavaScript's encodeURIComponent and encodeURI. Everything stays on your device.

## How it works
1. Paste the text or URL.
2. Choose Encode or Decode (and the scope).
3. Click Run, then copy the result.

## FAQ
### What is the difference between Component and Full URL?
Component encodes everything except letters, digits and - _ . ! ~ * ' ( ) : use it for a parameter value. Full URL also keeps separators like : / ? & = #.
### Why do spaces become “+”?
In HTML forms, “+” stands for a space. Enable the matching option (Pro mode) to decode it.
### Are accents handled?
Yes, as UTF-8: é becomes %C3%A9.
