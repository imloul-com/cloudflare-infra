# cloudflare-infra

Declarative Cloudflare infrastructure for the portfolio domain.

## Architecture

```
imloul.com/*
    │
    ▼
domain-router Worker (Rust, wasm)
    │
    ├── /tools/ast-viz/*  → worker-ast-viz Pages (prefix rewritten to `/`, <base> tag injected)
    └── everything else   → portfolio Pages (passthrough)
```

Two deployment pipelines, decoupled by design:

- **Terraform** manages the Worker Route binding (`imloul.com/* → domain-router`) and DNS/zone settings.
- **Wrangler** deploys the router Worker code independently — routing fixes ship in seconds without a Terraform cycle.

Apps (`ast-viz`, `portfolio`) deploy their own assets to Cloudflare Pages via their own repos. They have no knowledge of routing — all routing decisions live in `app-sources.json` in this repo. The CI fetches each app's `projectName` from its `wrangler.toml` at deploy time. App deploys auto-trigger a router redeployment via `repository_dispatch`.

## Sitemap strategy

The router Worker serves a domain-level sitemap index so search engines can discover URLs from multiple app origins under one domain:

- `https://imloul.com/sitemap.xml` (sitemap index)
- `https://imloul.com/sitemaps/{routeKey}.xml` (one child sitemap per entry in `worker/src/app-sources.json`)
- Current examples:
  - `https://imloul.com/sitemaps/portfolio.xml`
  - `https://imloul.com/sitemaps/ast_viz.xml`

For non-root prefixes (for example `ast_viz` at `/tools/ast-viz`), sitemap `<loc>` URLs are rewritten to the domain path (`https://imloul.com/tools/ast-viz/...`).

Search Console recommendation:

- Submit only `https://imloul.com/sitemap.xml`
- Do not submit child sitemaps separately unless debugging

## Directory layout

```
cloudflare-infra/
├── terraform/          # IaC: Worker Route, DNS, zone
│   ├── main.tf
│   ├── variables.tf
│   ├── outputs.tf
│   ├── versions.tf
│   └── backend.tf
├── worker/             # Router Worker (Rust)
│   ├── src/
│   │   ├── lib.rs      # Fetch entry point, health check, observability
│   │   ├── router.rs   # Route matching, proxying, <base> injection
│   │   ├── routes.rs   # Runtime route builder from ROUTE_DEFINITIONS env var
│   │   └── app-sources.json # Routing table + app repo references
│   ├── Cargo.toml
│   ├── Cargo.lock
│   └── wrangler.toml
└── .github/workflows/
    ├── terraform.yml       # Plan on PR (with comment), auto-apply on main
    └── deploy-worker.yml   # Test + deploy worker on changes to worker/
```

## Prerequisites

- Terraform >= 1.6
- Rust toolchain (stable) with `wasm32-unknown-unknown` target
- Node.js >= 22 (for Wrangler CLI usage)
- `CLOUDFLARE_API_TOKEN` env var

## Terraform usage

```bash
cd terraform
cp terraform.tfvars.example terraform.tfvars
cp backend.hcl.example backend.hcl
# edit both with your real values
export CLOUDFLARE_API_TOKEN="..."
export AWS_ACCESS_KEY_ID="..."
export AWS_SECRET_ACCESS_KEY="..."
terraform init -reconfigure -backend-config=backend.hcl
terraform plan
terraform apply
```

## Worker development

```bash
cd worker
cargo test           # run Rust unit tests
cargo check          # fast compile checks
make assemble-routes # fetch route configs from app repos (requires gh CLI)
npx wrangler dev     # local Worker dev
npx wrangler deploy  # production deploy
```

## GitHub Actions

### terraform.yml

- **PR**: `plan` → posts diff as PR comment
- **Push to main**: `plan` → `apply` (auto, gated by `production` environment)
- Triggered only by changes to `terraform/`

### deploy-worker.yml

- **PR**: `cargo test`
- **Push to main**: `cargo test` → assemble route configs from app repos → resolve origins → `wrangler deploy`
- Triggered by changes to `worker/`, `repository_dispatch` from app deploys, or manual dispatch

### Required repo settings

| Type     | Name                          |
|----------|-------------------------------|
| Secret   | `CLOUDFLARE_API_TOKEN`        |
| Secret   | `CLOUDFLARE_ACCOUNT_ID`       |
| Variable | `CLOUDFLARE_ZONE_NAME`        |
| Secret   | `TF_STATE_R2_BUCKET`          |
| Secret   | `TF_STATE_R2_ACCESS_KEY_ID`   |
| Secret   | `TF_STATE_R2_SECRET_ACCESS_KEY` |
| Secret   | `CROSS_REPO_TOKEN`            |

## Adding a new sub-app

1. Deploy the app to Cloudflare Pages (its own repo + workflow, with `wrangler.toml` defining the project name)
2. Add a `repository_dispatch` step to the app's deploy workflow (requires `INFRA_DISPATCH_TOKEN` secret)
3. Add one entry to `worker/src/app-sources.json` with `repo`, `routeKey`, `prefix`, and `rewriteTo`
4. Push to main — CI fetches the project name from the app's `wrangler.toml`, resolves origins, and deploys the router
