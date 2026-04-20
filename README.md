# cloudflare-infra

Declarative Cloudflare infrastructure and deployment orchestration for the portfolio domain.

## Architecture

```
imloul.com/*
    │
    ▼
domain-router Worker (Rust, wasm)
    │
    ├── /tools/ast-viz/*  → ast-viz Pages (prefix rewritten to `/`, <base> tag injected)
    ├── /tools/bloom-filter/*  → bloom-filter Pages (prefix rewritten to `/`, <base> tag injected)
    └── everything else   → portfolio Pages (passthrough)
```

This repository is the single source of truth for:

- DNS and Worker route binding (Terraform)
- app-to-route mapping
- Pages project names per environment
- deployment orchestration for app repos and router Worker

## Source of truth

`worker/src/app-sources.json` contains the app catalog. Each app entry declares:

- `appId`
- `registry.image`
- `deploy.prodVersion`, `deploy.devVersion`
- `route.routeKey`, `route.prefix`, `route.rewriteTo`
- `pages.projectName`, `pages.devProjectName`

The router and infra workflows derive route definitions and upstream origin resolution from this catalog only.

## Sitemap strategy

The router Worker serves a domain-level sitemap index so search engines can discover URLs from multiple app origins under one domain:

- `https://imloul.com/sitemap.xml` (sitemap index)
- `https://imloul.com/sitemaps/{routeKey}.xml` (one child sitemap per app that declares a `sitemap` path in `apps.yaml`)

Each app opts in by setting `sitemap: /path-to-sitemap.xml` on its entry in `apps.yaml`. Apps without the field are excluded from the index, and their `/sitemaps/{routeKey}.xml` path returns 404.

For non-root prefixes (for example `ast-viz` at `/tools/ast-viz`), sitemap `<loc>` URLs are rewritten to the domain path (`https://imloul.com/tools/ast-viz/...`).

## Directory layout

```
cloudflare-infra/
├── terraform/          # IaC: Worker Route, DNS, zone, Pages projects
├── worker/             # Router Worker (Rust)
│   ├── src/
│   │   ├── app-sources.json      # App catalog (single source of truth)
│   │   ├── bin/assemble_routes.rs
│   │   ├── bin/resolve_origins.rs
│   │   ├── bin/uptime_monitor.rs
│   │   └── bin/validate_catalog.rs
│   └── wrangler.toml
└── .github/workflows/
    ├── terraform.yml
    └── deploy-apps-and-router.yml
```

## Worker development

```bash
cd worker
cargo test
make validate-catalog
make assemble-routes ENVIRONMENT=prod
npx wrangler dev
```

## GitHub Actions

- `terraform.yml`: Terraform plan/apply for Cloudflare infra
- `deploy-apps-and-router.yml`: pulls versioned app artifacts from GHCR, deploys to Pages, then deploys router

## Required repo settings

| Type     | Name                                |
|----------|-------------------------------------|
| Secret   | `CLOUDFLARE_API_TOKEN`              |
| Secret   | `CLOUDFLARE_ACCOUNT_ID`             |
| Variable | `CLOUDFLARE_ZONE_NAME`              |
| Secret   | `TF_STATE_R2_BUCKET`                |
| Secret   | `TF_STATE_R2_ACCESS_KEY_ID`         |
| Secret   | `TF_STATE_R2_SECRET_ACCESS_KEY`     |
| Secret   | `CROSS_REPO_TOKEN` (read GHCR packages) |

## Adding a new sub-app

1. Add one app entry to `worker/src/app-sources.json`.
2. Ensure app CI publishes static artifacts to `registry.image` (`prod-latest`, `dev-latest`, and optional `sha-*` tags).
3. Ensure Terraform manages the matching Pages projects for prod/dev names.
4. Run `make validate-catalog` in `worker/`.
5. Trigger `deploy-apps-and-router.yml` (optionally with `version_override`).
