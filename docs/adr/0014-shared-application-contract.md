# 0014 — Shared Rust application contracts and structural checks

## Decision
Applications use AppContract in kortex-contract: the same ToolMeta, ports, limits, localized metadata and permission types as ToolContract, with the metadata section named app. Shared validation is reused; applications must declare web and desktop. A UI application can have no processing ports, while a processing tool must still declare an output. Metadata and top-level application typos are rejected rather than ignored.

xtask discovers apps/<name>/app.toml and generates registry/apps.json. Release metadata is read through that validated discovery instead of a second reduced TOML parser. The Store contract remains draft: declaring two intended targets does not prove both implementations or builds.

Structural checks require Rust engine/UI sources and the common app files, compare nonempty FR/EN locale keys and reject application TypeScript/Astro sources in apps/. App-named Astro product components in web/site are also refused; site integrations use generic components such as SiteSearch and AppPage. A synthetic Hub.astro fixture was rejected before checking the clean tree successfully. This prevents the known mistake; it is not a proof of every runtime/platform rule.

Store catalogue rules move to engine.rs and Dioxus presentation to ui.rs, using kortex-ui buttons and kortex-i18n catalogs. Existing OS access, native credentials, shared target features and the full runtime/backend checks remain explicit open plan items; this decision does not approve those existing gaps.
