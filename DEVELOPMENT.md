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

The [CI](.github/workflows/ci.yml) workflow runs `check`, `test:rust`, `test:js`, and
`test:fixtures:projects` on Linux for pull requests and pushes to `main`. It skips changes
that contain only documentation and version commits. A new push to a branch cancels the
running workflow for that branch.

The [Browser acceptance](.github/workflows/e2e.yml) workflow tests the `compatibility` tier
weekly. The Release workflow calls it for the `primary` tier. Start **Actions → Browser
acceptance → Run workflow** to test either tier manually.

`.github/workflows/actions.lock` records the commit of each action that the workflows use,
and GitHub Actions runs those commits. After you add a workflow or change a `uses:` value,
run `gh actions-lock` and commit the result. Install the command with
`gh extension install github/gh-actions-lock`. Review a change of a locked commit before
you commit it.

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

## Package releases

All publishable packages release at one shared version. `versioning.fixed` in
[pnpm-workspace.yaml](pnpm-workspace.yaml) lists them, and `versioning.ignore` lists every
private package. These lists accept exact names only, so add a new package to one of them.

Record each new feature and each other user-visible package change with
`vp exec pnpm change`, and commit the intent file with the change.
`vp exec pnpm change status` shows the release plan.

`vp exec pnpm version -r` consumes the intents, updates the `CHANGELOG.md` of each released
package, and creates no Git tag. The first release of each package does not use intents:
set its manifest to the intended version and publish it.

When `.changeset/` changes on `main`, the Version PR workflow runs `pnpm version -r` and
keeps the result in a pull request from `version/main`. `github-actions[bot]` creates this
pull request, so its Sites run waits for a maintainer to approve it in the Actions tab.
Merge the pull request with **Squash and merge**.

The merge updates `.changeset/ledger.yaml` on `main`. This change starts the Release
workflow, which builds and tests everything, including browser acceptance for the `primary`
tier. After these jobs succeed, the workflow uploads
the npm packages to npm's staging area and publishes the
[VS Code extension](packages/vscode-extension/DEVELOPMENT.md#publish). A version with a
prerelease suffix receives the npm tag `next`; any other version receives `latest`.

To check a release without uploading packages, start **Actions → Release → Run workflow**
on `main` with the default dry run. Clear **dry-run** to run the uploads again after a
failed release.

Staged versions are not installable until a maintainer approves them with 2FA. Review them
with `vp exec pnpm stage list`, then run `vp exec pnpm stage approve` and select every
package. pnpm approves packages in dependency order and skips a package when one of its
workspace dependencies is not approved. Use `vp exec pnpm stage reject <stage-id>` to
remove a staged version. After the approval succeeds, advance `stable` to the release
commit.

### npm setup

1. Create the `npm-release` GitHub environment with a **Branch** rule for `main`.
2. For a package that does not exist on npm yet, add a short-lived granular token as the
   `NPM_TOKEN` environment secret.
3. After the first publication, add this repository's `release.yml` and the `npm-release`
   environment as the trusted publisher of each package, allow staged publishing for it,
   then delete `NPM_TOKEN`.

npm adds provenance only to trusted publications from a public repository.

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
subdomain, so pull request 123 uses `https://pr-123.preview.stillwind.dev/`
and `https://pr-123.play-preview.stillwind.dev/`.

### Pull request Previews

Pull requests do not publish Previews by default. Two controls request one:

| Control                  | Who can use it            | Pull requests            | Effect                          |
| ------------------------ | ------------------------- | ------------------------ | ------------------------------- |
| `preview` label          | People with triage access | This repository          | Publishes each new commit       |
| `/preview` comment       | People with write access  | This repository          | Publishes the current head once |
| `/preview <sha>` comment | People with write access  | This repository or forks | Publishes that head commit once |

The `<sha>` value must be the full SHA of the current head commit. The label has no
effect on a fork, because a fork can add commits after a maintainer reviews it. A fork's
Sites run also needs approval, and that run must finish before `/preview <sha>`.

Sites builds each pull request with these Preview origins for the sites in its change
plan. Links to other sites use the `main` Preview. The
[Site Preview](.github/workflows/site-preview.yml) workflow publishes that build without
rebuilding it, so a Preview requires a build from the last 7 days. It accepts builds
from failed runs, because a Preview can help to investigate the failure.

Site Preview runs from the default branch and holds the Cloudflare token, so it never
executes pull request code. Workflow files in a pull request can change what its runs
report and upload. The workflow therefore reads labels, head commits, and permissions
from the GitHub API. It takes only static assets from the artifact, and it writes the Worker
configuration from the default branch. [Site Preview Request](.github/workflows/site-preview-request.yml)
only starts Site Preview after labeling, because public repositories disable
`pull_request_target` by default.

The workflow reports the URLs in one comment on the pull request and adds the
`preview:active` label. A daily run deletes the Previews of closed pull requests that
have this label. Removing the `preview` label stops updates and keeps the Preview until
the pull request closes.

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
configured Preview URLs on `main`, the production URLs on `stable`, and the URLs
described in [Pull request Previews](#pull-request-previews) on pull requests.
`check:urls` checks the generated files before deployment. The deployment checks also
compare the published canonical and social card URLs with the actual deployment URL.

The lockfile fixes the versions of Cloudflare CLI `cf` and its Build Output libraries. Update
them together and verify both deployment modes when upgrading during the beta.
The deployment job downloads the checked Build Output and uses `cf` with
`--prebuilt`. It does not build application code. Only that step receives the
Cloudflare token. Pull request runs have no deployment credentials.
The GitHub deployments and Actions summary link to the deployed sites.
Deployments to the same Worker run one at a time across both branches.
HTTP checks verify documentation routes, Preview indexing rules, and WASM headers
after deployment. The playground acceptance suite tests browser behavior before deployment.
The workflow builds the selected sites in one job. Six browser jobs cover formatting and
completions, compiler and TypeScript diagnostics, HMR, shared links, file loading, and worker
recovery. They consume the same acceptance build and use independent browser state.
A failed scenario can reuse that build when retried. `Build and verify` always runs
and requires every selected job to succeed.
It accepts a skipped job only when the change plan omitted it.
Production validation consumes the Build Output prepared earlier in the build job.

`tools/workspace/impact.ts` resolves workspace packages and their transitive consumers.
The Sites plan adds Rust and generator inputs and selects each application's checks.
Website-only changes skip the playground build and browser jobs. Browser test changes
select their scenarios without deploying. Developer documentation and unrelated packages
skip site work. Unclassified inputs, missing Git history, and manual runs select both sites
and all scenarios. PRs compare against their base. Pushes compare against the last successful Sites run
on the branch, so failed or cancelled runs cannot hide unpublished changes. An obsolete
run fails its deployment head check and cannot become that baseline.
The plan appears in the Actions summary. It rejects a browser matrix that does not cover
every playground test file; register new scenarios in `tools/sites/ci.ts`.

`verify:sites:website` and `verify:sites:play` run the corresponding build checks locally.
Package builds and unit or framework tests can use the same impact helper to select their
own jobs. The Sites workflow checks only its applications and their required artifacts.

The build job restores Vite Task results after installing dependencies. Task fingerprints
track sources and deployment URLs before reusing outputs. Cargo and Vite Task use a new
GitHub cache key for each run, so successful builds save their updated caches.
Cargo archives are saved only when the build changes files under `target/`.

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
2. Under **Settings → Environments**, create `sites-preview`, `sites-pr-preview`, and
   `sites-production`. For each environment, set **Deployment branches and tags**
   to **Selected branches and tags**. Add a **Branch** rule for `main` in
   `sites-preview` and `sites-pr-preview`, and a **Branch** rule for `stable` in
   `sites-production`. Create the `preview` and `preview:active` labels.
3. Add `CLOUDFLARE_API_TOKEN` under **Environment secrets** in each environment.
   All can initially use the token created above. Add `CLOUDFLARE_ACCOUNT_ID`
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

## Package versions

All publishable packages share one version until `1.0.0`. In `0.x` releases, breaking
changes use `minor` and other changes use `patch`. `versioning.maxBump: minor` stops an
accidental `major` bump, so remove that setting when you release `1.0.0`.

After `1.0.0`, `stillwind`, `@stillwind/native` and its platform packages,
`@stillwind/config`, `@stillwind/projection`, and `@stillwind/host` stay in the fixed
group. `@stillwind/integration`, `@stillwind/analysis`, `@stillwind/cli`, and
`@stillwind/language-server` also stay there until they need separate releases.

`@stillwind/plugin-iconify`, `prettier-plugin-stillwind`, and
`@stillwind/vscode-extension` can leave the fixed group when two conditions hold: the
public APIs they depend on are stable, and they need releases apart from the other
packages. A package that leaves the group keeps its current version and declares the
supported `stillwind` versions in `peerDependencies`.

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
