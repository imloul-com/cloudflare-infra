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

Apps (`ast-viz`, `portfolio`) deploy their own assets to Cloudflare Pages via their own repos. They have zero knowledge of deployment paths or routing.

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
│   │   ├── routes.rs   # Runtime route builder from route-definitions.json
│   │   └── route-definitions.json # Single source of truth for routes/projects
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
cargo test        # run Rust unit tests
cargo check       # fast compile checks
npx wrangler dev  # local Worker dev
npx wrangler deploy # production deploy
```

## GitHub Actions

### terraform.yml

- **PR**: `plan` → posts diff as PR comment
- **Push to main**: `plan` → `apply` (auto, gated by `production` environment)
- Triggered only by changes to `terraform/`

### deploy-worker.yml

- **PR**: `cargo test`
- **Push to main**: `cargo test` → `wrangler deploy`
- Triggered only by changes to `worker/`

### Required repo settings

| Type     | Name                          |
|----------|-------------------------------|
| Secret   | `CLOUDFLARE_API_TOKEN`        |
| Secret   | `CLOUDFLARE_ACCOUNT_ID`       |
| Variable | `CLOUDFLARE_ZONE_NAME`        |
| Secret   | `TF_STATE_R2_BUCKET`          |
| Secret   | `TF_STATE_R2_ACCESS_KEY_ID`   |
| Secret   | `TF_STATE_R2_SECRET_ACCESS_KEY` |

## Adding a new sub-app

1. Deploy the app to Cloudflare Pages (its own repo + workflow)
2. Add one entry to `worker/src/route-definitions.json` with `prefix`, `projectName`, `originVar`, and `rewritePrefixTo`
3. Push to main — CI resolves the real `*.pages.dev` subdomain dynamically and deploys the worker with the new route
