# cloudflare-infra

Declarative Cloudflare infrastructure for:

- Pages project: `portfolio`
- Pages project: `worker-ast-viz`
- Stable custom domains:
  - `portfolio.<your-zone>`
  - `ast-viz.<your-zone>`
- Router Worker on `<your-zone>/*`:
  - `/tools/ast-viz/*` -> `ast-viz.<your-zone>`
  - everything else -> `portfolio.<your-zone>`

## Prerequisites

- Terraform `>= 1.6`
- Cloudflare API token in env var:
  - `CLOUDFLARE_API_TOKEN`

## Usage

```bash
cd terraform
cp terraform.tfvars.example terraform.tfvars
cp backend.hcl.example backend.hcl
# edit terraform.tfvars with your account_id and zone_name
export CLOUDFLARE_API_TOKEN="..."
export AWS_ACCESS_KEY_ID="..."      # R2 API token access key
export AWS_SECRET_ACCESS_KEY="..."  # R2 API token secret
# edit backend.hcl with your real bucket + account endpoint
terraform init -reconfigure -backend-config=backend.hcl
terraform plan
terraform apply
```

Why `terraform init` asks for `bucket`:

- Backend settings are loaded before Terraform input variables.
- `terraform.tfvars` / `TF_VAR_*` do not configure backend fields.
- Put backend values in `backend.hcl` (or pass `-backend-config` flags).

R2 backend auth note:

- Use R2 S3 credentials for backend auth:
  - `AWS_ACCESS_KEY_ID` = R2 Access Key ID
  - `AWS_SECRET_ACCESS_KEY` = R2 Secret Access Key
- Do not use `CLOUDFLARE_API_TOKEN` for the S3 backend.
- Ensure backend config includes `skip_requesting_account_id = true`.

## Notes

- This repo manages infra only (projects, domains, DNS, router worker, route).
- App repos still build/deploy content to their Pages projects.
- Keep project names aligned with app deploy workflows:
  - `portfolio`
  - `worker-ast-viz`

## GitHub Actions

Workflow file: `.github/workflows/terraform.yml`

- Pull request / push to `main`: runs `init`, `fmt`, `validate`, and `plan`
- Manual apply: run workflow dispatch with `apply=true`

Required repository settings:

- Secret: `CLOUDFLARE_API_TOKEN`
- Secret: `CLOUDFLARE_ACCOUNT_ID`
- Variable: `CLOUDFLARE_ZONE_NAME`
- Secret: `TF_STATE_R2_BUCKET`
- Secret: `TF_STATE_R2_ACCESS_KEY_ID`
- Secret: `TF_STATE_R2_SECRET_ACCESS_KEY`

Remote state notes:

- Create an R2 bucket dedicated to Terraform state (example: `tf-state-cloudflare-infra`).
- Keep `.terraform.lock.hcl` committed in git for reproducible provider versions.
