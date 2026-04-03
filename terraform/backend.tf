terraform {
  # Centralized remote state in Cloudflare R2 (S3-compatible backend).
  # Intentionally partial: concrete values are passed via -backend-config
  # from local shell / CI secrets.
  backend "s3" {}
}
