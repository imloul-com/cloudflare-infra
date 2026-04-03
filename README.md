# cloudflare-infra

Declarative Cloudflare infrastructure for:

- **Router Worker** on `imloul.com/*`:
  - `/tools/ast-viz/*` → ast-viz Pages project (prefix stripped, `__ROUTER_BASE__` injected)
  - everything else → portfolio Pages project
- **Pages project discovery** — resolves real `*.pages.dev` hostnames at plan time
- **Worker route** — binds the router to the root domain

The router worker source lives in `worker/router.js.tmpl` and is deployed by Terraform.
App repos (`ast-viz`, `portfolio`) only deploy their own content to Pages — routing is managed here.

## Prerequisites

- Terraform `>= 1.6`
- `CLOUDFLARE_API_TOKEN` env var

## Usage

```bash
cd terraform
cp terraform.tfvars.example terraform.tfvars
cp backend.hcl.example backend.hcl
# edit both with your real values
export CLOUDFLARE_API_TOKEN="..."
export AWS_ACCESS_KEY_ID="..."      # R2 API token access key
export AWS_SECRET_ACCESS_KEY="..."  # R2 API token secret
terraform init -reconfigure -backend-config=backend.hcl
terraform plan
terraform apply
```

R2 backend auth:
- `AWS_ACCESS_KEY_ID` / `AWS_SECRET_ACCESS_KEY` = R2 S3 credentials (not `CLOUDFLARE_API_TOKEN`)
- Backend config must include `skip_requesting_account_id = true`

## GitHub Actions

Workflow: `.github/workflows/terraform.yml`

- PR / push to `main`: `init` → `fmt` → `validate` → `plan` (artifact saved)
- Manual dispatch with `apply=true`: downloads plan artifact → `apply`

Required repo settings:

| Type     | Name                         |
|----------|------------------------------|
| Secret   | `CLOUDFLARE_API_TOKEN`       |
| Secret   | `CLOUDFLARE_ACCOUNT_ID`      |
| Variable | `CLOUDFLARE_ZONE_NAME`       |
| Secret   | `TF_STATE_R2_BUCKET`         |
| Secret   | `TF_STATE_R2_ACCESS_KEY_ID`  |
| Secret   | `TF_STATE_R2_SECRET_ACCESS_KEY` |

## Notes

- The separate `domain-router` repo is no longer needed — archive it.
- Keep Pages project names aligned with app deploy workflows: `portfolio`, `worker-ast-viz`.
- Commit `.terraform.lock.hcl` for reproducible provider versions.
