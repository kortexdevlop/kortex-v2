# Changelog

## 0.3.2
- Reads the signed contract catalogue published by kortex.site (verified, cached for offline use): a tool added later appears without updating this app. Desktop lists the tools really installed, including ones this build does not know.

## 0.3.1
- Desktop: rebuilt with the shared file-picker fix of the Kortex UI.

## 0.3.0
- Desktop: install, update, remove and open tools and Kortex apps from the signed index, with the device account and password confirmation (same rules as `kortex install`).
- Web: link to the desktop download page instead of the outdated "in preparation" notice.

## 0.2.0
- Share the Rust/Dioxus Store component between web and desktop.
- Add contract-driven catalogue cards, search, universe filters, detail sheets and declared permissions.
- Detect actual local installations on desktop and open installed tools.
- Keep public installation closed until secure credentials and distribution are verified.

# Changelog

## Unreleased
- Adopt the shared application contract and separate pure catalogue rules from Dioxus components.
- Remain draft until shared web/desktop execution and platform access are validated.
