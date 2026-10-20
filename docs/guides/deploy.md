# Static publication

> **Voie nominale (ADR 0034)** : `feed.yml` publie les hôtes statiques depuis l'index signé fusionné (`xtask restore-web`), avec les secrets GitHub `CLOUDFLARE_API_TOKEN` et `CLOUDFLARE_ACCOUNT_ID`, sans recompiler. Le reste de ce guide décrit le secours local reproductible et la voie `deploy.yml` (manuelle, depuis un run `release.yml`).

The current public scope is the Astro site, the web tools of the signed catalogue and the Rust apps. The account API is deployed with D1. Desktop downloads have been public since 2026-10-19 (`desktop_releases = true`): build the site with `KORTEX_RELEASE_DIR` pointing at the files of the current public GitHub Release, never at a local test index (`web/site/scripts/sync.mjs` refuses it). The tool catalogue is frozen except for explicit user authorisations (RULES §2.1).

1. Build only changed tool bundles with `cargo xtask build <id…>` when engine/UI sources changed.
2. Provide Cloudflare credentials through Wrangler's normal authentication or `CLOUDFLARE_API_TOKEN` and `CLOUDFLARE_ACCOUNT_ID`. Never save credentials in source files.
3. Run `pnpm run deploy`. This rebuilds Astro, prepares public artifacts, prints the inventory, applies pending account migrations, publishes the API and checks its health, then publishes the tool host followed by the site host.
4. Check HTTPS, one complete tool execution, search, the account flow and the `/telecharger` page with the index served under `/registry/` identical to the release. Record versions and results in `docs/PROGRESS.md`.

Domains come from `brand/domains.toml`; `scripts/prepare-publish.mjs` generates the static Wrangler configurations from the site/tool/app inventory. The staging directories are `dist/publish/site` and `dist/publish/run`. Local desktop release test feeds, installation scripts, hidden files, source maps and Rust sources are excluded or rejected. File size/count checks enforce the static asset free-plan limits verified on 2026-10-16.

HTTP security headers are generated with the artifacts. Site inline scripts use content hashes. The isolated Dioxus tool origin needs `unsafe-eval` because Dioxus 0.7.10 implements document evaluation with `Function::new_with_args`; it does not receive account cookies. The account API remains a separate origin. Do not add an X-Frame-Options DENY/SAMEORIGIN header to the tool host: it must accept the site iframe.

Each deployment creates a version on Cloudflare. Record version IDs for rollback. After the first deployment, use `wrangler rollback <previous-version-id> --config services/<site|run>/wrangler.jsonc` only after inspecting the target version and obtaining authorization for the intended rollback. Never delete Workers/DNS/D1 as a rollback shortcut.

The user authorized publication of each finished batch, superseding the earlier instruction not to deploy. No paid services or resource cleanup are part of this command. The account database was created separately after inventory; it is never recreated by a deployment.

Sources: [Cloudflare static asset limits](https://developers.cloudflare.com/workers/static-assets/billing-and-limitations/), [static response headers](https://developers.cloudflare.com/workers/static-assets/headers/).


## Account availability
`brand/features.toml` persists public feature availability. Build variables `PUBLIC_ACCOUNT_ENABLED` and `PUBLIC_DESKTOP_RELEASES_ENABLED` can override it explicitly. The account flag also opens the site CSP connection to the API origin. Never activate desktop releases using the local test index; provide the verified public feed through `KORTEX_RELEASE_DIR`.

The D1 production binding lives under `env.production` in the API config (bindings are not inherited from the local environment). Production uses Secure host-only cookies, an origin allow-list excluding the tool host, and a daily 03:17 UTC expiry sweep. AI has no production binding and returns unavailable without consuming model credits.

Run the grouped `scripts/tests/published-smoke.mjs` once after publication. When accounts are enabled it creates and deletes only its own QA account and verifies signup, password handling, session persistence, settings, export and erasure. Existing user accounts are never used.

Pour un lot limité au site (API et outils inchangés), préparer avec `pnpm site:build` puis `node scripts/prepare-publish.mjs`, et publier via `node scripts/publish.mjs --site-only`. La vérification de santé API reste obligatoire si le compte est actif.


For changes to service-worker routing or iframe hosting, run the grouped unit tests `node --test scripts/tests/offline-routing.test.mjs scripts/tests/host-fallback.test.mjs` and the production recipe `node scripts/tests/offline-smoke.mjs`. This hosting recipe creates no account; it verifies offline requests, a cached account shell, iframe origin and live theme propagation. A real Firefox run remains distinct from the isolated fallback test.

### Account security

Passkeys use `WEBAUTHN_RP_ID` and `WEBAUTHN_ORIGIN`, generated from the root domain by `prepare-publish.mjs`. Migration 0005 only adds credential metadata and expiring challenges. Password rotation is an atomic D1 batch that invalidates old browser sessions, device tokens and pending enrollment requests; registered passkeys remain available.

Turnstile is prepared but disabled while `brand/security.json` contains an empty public site key. A previous custom-token provisioning attempt received HTTP 403 from the Turnstile widget API; recheck the current credential scopes before retrying. Activation requires account-level **Turnstile Sites Write** permission (or provisioning the widget separately), an exact `kortex.site` hostname restriction, the `register` action, and the secret installed with `wrangler secret put TURNSTILE_SECRET_KEY --env production --config services/api/wrangler.jsonc`. Never commit that secret. Then save the public site key in `brand/security.json`, update the privacy policy to describe the Cloudflare challenge, rebuild and publish API/site together. The generated CSP allows the Cloudflare script/frame only when a real site key is configured. Missing server secret/token or a failed action/hostname check rejects registration when enabled.

Grouped account security recipe (test-only Playwright runtime): `node scripts/tests/account-security.mjs`. It uses a fresh disposable QA account and a virtual authenticator, verifies credential enrollment/login/revocation, password rotation and session revocation, then deletes only its own account. `KORTEX_QA_FILE` can supply a pre-created disposable account from a private temporary file when signup is protected; that file is removed after successful cleanup. A real phone/security key still needs a manual compatibility check.

## Shared platform batches and static-only publication

A shared runtime/UI/platform change affects all web bundles: run `cargo xtask build all`, let source/output receipts skip identical builds, and finish the grouped `node scripts/tests/platform-ui-smoke.mjs` recipe before publishing. It needs the existing test-only Playwright runtime and Chromium; set `NODE_PATH`/`CHROME_PATH` for that runtime when necessary. Native sampling is separate from public native release acceptance.

For a site/tool asset batch with an unchanged API, prepare the current site build and rebuilt tool outputs, then use the following sequence. This publishes the tool host before the site and keeps the API health check without applying migrations or redeploying the API:

```sh
node scripts/prepare-publish.mjs
wrangler deploy --config services/run/wrangler.jsonc
node scripts/publish.mjs --site-only
```

Wrangler can use an already connected local OAuth session. Check `wrangler whoami` before publishing. An explicit invalid `CLOUDFLARE_API_TOKEN` overrides that session; remove that override to use the existing login. Never print credentials or place them in command arguments. The static publication of the platform batch on 2026-10-17 uses the existing OAuth session after the previous custom token was rejected.

## Accidental deletion recovery (2026-10-17)

After the user's restoration request, the original D1 database was recovered using its configured UUID through Cloudflare's [undelete API](https://developers.cloudflare.com/api/resources/d1/subresources/database/methods/undelete/). The restored database had no pending migration; API/site were redeployed with their existing bindings and public domains. No empty database was created. See PROGRESS.md for versions and validation.

The public DNS records were correct before the machine's negative DNS cache expired. Test-only `KORTEX_QA_HOST_RULES` may supply current verified public records to the Chromium recipes; never disable TLS validation to work around DNS. Record such a test configuration as distinct from the ordinary system resolver. An additional local SQL export was refused by automatic approval review; it was not performed and must not be retried without explicit authorization for that destination.


## Fresh account database (2026-10-17)

The subsequent explicit user answer, “Recréer une base vide”, superseded the restored production binding. The active database is now kortex-accounts-fresh-20261017 (14db05e6-f667-4423-a289-946a0f18ebca), initialized with all five migrations. The recovered kortex-accounts database remains remotely preserved and unbound; no old accounts were imported and no SQL export was performed. New account registration is required. The production recipe passed and left zero accounts after deleting its disposable QA account.

The publisher applies migrations using the configured DB binding, so a future database name change does not silently target the old database. Keep database creation separate from deployment. See validation/2026-10-17-fresh-account-database.json for the active API version and evidence; the earlier platform snapshot describes the historical deployment.

## Rust application static delivery

Applications with a web target and a beta/stable contract are discovered from `registry/apps.json`. Their hostname must exist in `brand/domains.toml`. Build the shared Rust packages before preparing the site; source/output receipts and SHA-256 file manifests are written to `.kortex/app-receipts/`.

```sh
cargo xtask build-app apps.hub --web
cargo xtask build-app apps.desk --web
cargo xtask build-app apps.hub
cargo xtask build-app apps.desk
# Compile affected tool bundles and verify their receipts before publication.
cargo xtask build all
pnpm site:build
node scripts/prepare-publish.mjs
node scripts/publish.mjs --static-only
```

`--static-only` checks API health and publishes static targets without applying migrations or deploying the account Worker. The run CSP permits the configured site/Desk/Hub ancestors; app CSP permits only the configured run frame origin. No source maps, Rust/config sources or credentials enter static assets. Desktop binaries are local compilation evidence, not a public download release. Windows/Linux and interactive native acceptance remain separate requirements.

## GitHub static publication workflow

`deploy.yml` is manual and accepts a completed `release.yml` run number. Its web job must have succeeded even if an unrelated desktop job failed. Preparation downloads only that run's web artifact, requires an exact 38-tool/app inventory and unchanged compiled sources, verifies output fingerprints and app SHA-256 manifests, then builds the independent Astro site. It never recompiles the desktop catalogue.

The default `publish=false` prepares an artifact only: `prepared-static-publication`, retained seven days, contains the public assets, generated static Worker configurations and build provenance. This is not a public preview or a deployment. Rust/Node build-only receipt checks add no product dependency. A source difference in crates/tools/apps/xtask/design/contracts fails closed; build a new release web artifact first.

With `publish=true`, a separate job requires `CLOUDFLARE_API_TOKEN` and `CLOUDFLARE_ACCOUNT_ID` GitHub secrets. Provision a scoped static Worker deployment token; do not copy a local OAuth session or place tokens in workflow inputs. These secrets are currently absent. The publisher checks the existing account API, publishes run/apps/site without D1 migrations or API changes, verifies all 41 public entry hashes, and uploads `static-publication-result` with source/publication commits and deployed version IDs. Desktop downloads stay controlled by the existing feature flag.

Pull requests changing the deployment workflow/verifier also run preparation against the actual published build recorded in `.github/web-build.json`. This verifies the workflow before merge without any production secret. Such PR runs cannot enter the publication job. Update the build reference only after a new release web job has completed and its artifacts are available.
