# Development

This repository contains the Rust compiler, JavaScript host services, build
integrations, and developer tools. The workspace uses pnpm through Vite+.
Tasks are defined in each package's `vite.config.ts#run.tasks`.

## Setup and verification

Install Vite+, Rust, and the platform linker. The required Rust version is declared
in [Cargo.toml](Cargo.toml). Vite+ uses the pnpm version selected by the workspace.

Run from the repository root:

```sh
vp i --frozen-lockfile
vp run verify
```

`verify` runs `check` and `test`. Browser, WASM, framework acceptance, and timing
checks have separate tasks and prerequisites. See [Tests and verification](docs/test-inventory.md)
for the task inventory, generated files, cache behavior, and fixture workflows.
Run generation tasks explicitly and review their output before comparing fixtures.

Package build tasks build their dependencies through the task graph. Applications
that use the workspace integration declare `dependsOn: ["@stillwind/integration#build"]`
for their development and build tasks. This prerequisite builds the integration's
`dist/` files before the application loads them.

Before you publish `stillwind`, run `vp run stillwind#prepare-publish`. It builds the
package and the website, and it copies the website's Markdown pages into
`packages/core/docs/`, which git ignores.

Before you publish the platform packages of `@stillwind/native`, run
`vp run @stillwind/native#prepare-publish`. It checks that
`packages/native/RUST-THIRD-PARTY-LICENSES` is current and copies it into each
platform package, where git ignores the copy.

## Site deployments

[Sites](.github/workflows/sites.yml) builds and checks the website and playground
on pull requests and pushes to `main` and `stable`. Pushes to `main` update the
`main` Preview of each Worker. Pushes to `stable` deploy production:

| Application | Worker              | Production URL               |
| ----------- | ------------------- | ---------------------------- |
| Website     | `stillwind-website` | `https://stillwind.dev`      |
| Playground  | `stillwind-play`    | `https://play.stillwind.dev` |

The `main` Preview uses `https://main.preview.stillwind.dev/` for the website
and `https://main.play-preview.stillwind.dev/` for the playground. Each Worker
has a dedicated parent domain for Previews. A Preview name becomes its first
subdomain, so future PR Previews can use names such as `pr-123`.

Until the first package release, `stable` tracks the selected initial site
snapshot. It then contains the code and documentation for the published packages. Apply
documentation fixes there and bring them into `main`. After a package release
succeeds, advance `stable` to its release commit. If automation updates `stable`
with `GITHUB_TOKEN`, explicitly dispatch Sites on `stable`: pushes made with that
token do not start another workflow.

The applications declare their Worker settings in `cloudflare.config.ts`.
Vite+ builds the static sites and `tools/sites` writes Cloudflare Build Output
under `.deploy/<app>/.cloudflare/output/v0/`. The build excludes source maps and
internal build files, checks asset limits, and preserves WASM isolation headers.
Preview builds add `noindex` and block crawlers in `robots.txt`.
`PUBLIC_WEBSITE_URL` and `PUBLIC_PLAYGROUND_URL` select the two origins used by
metadata, sitemaps, agent indexes, and navigation between the sites. CI uses the
configured Preview URLs on `main` and pull requests, and the production URLs on `stable`.
`check:urls` checks the generated files before deployment. The deployment checks also
compare the published canonical and social card URLs with the actual deployment URL.

The lockfile fixes the versions of Cloudflare CLI `cf` and its Build Output libraries. Update
them together and verify both deployment modes when upgrading during the beta.
The deployment job downloads the checked Build Output and uses `cf` with
`--prebuilt`. It does not build application code. Only that step receives the
Cloudflare token. Pull requests run without deployment credentials.
The GitHub deployments and Actions summary link to the deployed sites.
Deployments to the same Worker run one at a time across both branches.
HTTP checks verify documentation routes, Preview indexing rules, and WASM headers
after deployment. The playground acceptance suite tests browser behavior before deployment.
The workflow keeps builds and browser acceptance in separate jobs, so a failed browser
job can use the same build when retried. `Build and verify` requires both jobs to pass.

To inspect a browser failure, download its evidence artifact and open `trace.zip`
with Playwright Trace Viewer. To test a downloaded browser assets artifact locally,
set `STILLWIND_PLAY_TEST_DIRECTORY` to its extracted directory and run `vp test --run`
from `e2e/play`. This runs the downloaded build without rebuilding it.

### Cloudflare setup

1. Activate the `stillwind.dev` zone in the target Cloudflare account and choose
   its `workers.dev` subdomain. GitHub Actions manages builds and deployments;
   leave Workers Builds disconnected from this repository.
2. Create a token scoped to that account. Grant Workers **Admin** at the product
   scope for initial Worker creation, plus **Workers Routes Write** for the
   `stillwind.dev` zone. After provisioning, Workers **Editor** is sufficient
   when deployments keep the existing Worker and domain connections.
3. Record the account ID. The first Preview deployment initializes each Worker
   without production routes, because Custom Domains require an existing deployment.
   It attaches `preview.stillwind.dev` and `play-preview.stillwind.dev` for Preview
   traffic only. Cloudflare provisions wildcard DNS records and certificates.
   Later Preview deployments preserve the existing production deployment.
   The first deployment from `stable` attaches the production Custom Domains.
4. Verify HTTPS on both `main` Preview URLs after certificate issuance. Preview
   builds explicitly send `X-Robots-Tag: noindex` on custom domains.

See [Workers permissions](https://developers.cloudflare.com/workers/authorization/workers/)
and [Preview Custom Domains](https://developers.cloudflare.com/workers/previews/custom-domains/).

### GitHub setup

1. Enable GitHub Actions and allow the actions pinned in `sites.yml`.
2. Under **Settings → Environments**, create `sites-preview` and
   `sites-production`. For each environment, set **Deployment branches and tags**
   to **Selected branches and tags**. Add a **Branch** rule for `main` in
   `sites-preview` and a **Branch** rule for `stable` in `sites-production`.
3. Add `CLOUDFLARE_API_TOKEN` under **Environment secrets** in each environment.
   Both can initially use the token created above. Add `CLOUDFLARE_ACCOUNT_ID`
   as a repository variable under **Settings → Secrets and variables → Actions**.
   Add repository variables `SITES_PREVIEW_WEBSITE_URL` and `SITES_PREVIEW_PLAYGROUND_URL`
   with the `main` Preview URL of each Worker. These must be absolute HTTPS origins.
   They are `https://main.preview.stillwind.dev/`
   and `https://main.play-preview.stillwind.dev/`.
   Remove any repository secret with the same token name after configuring both
   environments, so deployments require the environment secret.
4. Require the `Build and verify` check on pull requests to `main` and `stable`
   and disable force pushes on `stable`.
5. Merge the workflow into `main` to deploy Preview. Create `stable` from the
   reviewed commit and push it to deploy production. Later runs can also be
   started from **Actions → Sites → Run workflow**, selecting `main` or `stable`.

Both environments deploy automatically after their checks pass. GitHub Pro
supports environments, environment secrets, and deployment branch rules for
private repositories. Required reviewers and wait timers on that plan require
a public repository. See [Managing environments](https://docs.github.com/en/actions/how-tos/deploy/configure-and-manage-deployments/manage-environments).

For local verification, set `PUBLIC_WEBSITE_URL` and `PUBLIC_PLAYGROUND_URL` to
the intended origins and set `SITE_DEPLOYMENT` to `preview` or `production`,
then run `vp run verify:sites`. Both URLs default to the production origins.
Use localhost origins to test navigation between local development servers.
Production output also supports `vp run @stillwind-internal/tools-sites#check:production`,
which performs a `cf deploy --prebuilt --dry-run` without credentials.
To deploy prepared output locally, set `CLOUDFLARE_ACCOUNT_ID` and
`CLOUDFLARE_API_TOKEN`, then run
`vp run @stillwind-internal/tools-sites#deploy -- website main` or select `play`
and `stable` as needed. The branch argument must match the prepared mode.

To roll back a site, select the previous version in that Worker's Cloudflare
Deployments dashboard. Each Worker is deployed and rolled back separately.
Repository transfers also require reviewing Actions policies and token access
in the destination organization.

## Repository layout

| Directory     | Responsibility                                                                 |
| ------------- | ------------------------------------------------------------------------------ |
| `crates/`     | Compiler, CSS design systems, source models, project sessions, and bindings    |
| `packages/`   | Public application APIs, build integrations, host services, and editor tooling |
| `plugins/`    | Optional utility plugins                                                       |
| `apps/`       | Website, playground, fixture viewer, and design tools                          |
| `e2e/`        | Browser acceptance across frameworks and bundlers                              |
| `fixtures/`   | Reviewed compiler output and Tailwind compatibility cases                      |
| `benchmarks/` | Explicit performance measurements and budgets                                  |
| `tools/`      | Generated data, fixtures, and shared task configuration                        |

Publishable packages use `stillwind`, `@stillwind/*`, or `prettier-plugin-stillwind`.
Private applications, acceptance fixtures, and tools use `@stillwind-internal/*`.
The root package is `@stillwind-internal/root`.

README files describe package usage. Internal README files state their role and
link to the relevant contracts. Keep repository workflows in this document,
package-specific development details in `DEVELOPMENT.md`, and shared contracts in
`docs/`. Record unresolved design and validation work in [docs/TODO.md](docs/TODO.md).

## Contracts and development guides

| Document                                                      | Responsibility                                                                |
| ------------------------------------------------------------- | ----------------------------------------------------------------------------- |
| [Architecture](docs/crate-architecture.md)                    | Crate ownership, service entrypoints, diagnostics, and the Node binding       |
| [Macros](docs/macro-contract.md)                              | Accepted inputs, purity, composition, refs, animations, and custom properties |
| [CSS ordering](docs/css-ordering.md)                          | Declaration, module, and delivery precedence                                  |
| [CSS emission](docs/css-emission.md)                          | Linking, output slots, explicit values, and minification                      |
| [Generated identifiers](docs/generated-identifiers.md)        | Identity, naming callbacks, escaping, and collisions                          |
| [Runtime](docs/runtime.md)                                    | Lowering and framework materialization                                        |
| [Analysis](docs/standalone-analysis.md)                       | Configuration, analysis services, lint rules, and editor contracts            |
| [SFCs](docs/sfc-type-checking.md)                             | Projection, checker facts, and source coordinates                             |
| [Host services](packages/host/DEVELOPMENT.md)                 | Module ownership and editor preview lifetimes                                 |
| [Build integrations](packages/integration/DEVELOPMENT.md)     | Adapter lifetimes, transport, and verification                                |
| [VS Code extension](packages/vscode-extension/DEVELOPMENT.md) | Desktop/Web bundles and VSIX packaging                                        |
| [Browser acceptance](e2e/DEVELOPMENT.md)                      | Fixture execution, manifests, and artifacts                                   |
| [Benchmarks](benchmarks/DEVELOPMENT.md)                       | Workloads and measurement boundaries                                          |

Private applications document their local workflows beside their sources:
[Website](apps/website/DEVELOPMENT.md), [Playground](apps/play/DEVELOPMENT.md), and
[Shape Lab](apps/logo-editor/DEVELOPMENT.md).
