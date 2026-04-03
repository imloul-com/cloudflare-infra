variable "account_id" {
  description = "Cloudflare account ID"
  type        = string
}

variable "zone_name" {
  description = "Root zone managed in Cloudflare (example: imloul.com)"
  type        = string
}

variable "router_worker_name" {
  description = "Cloudflare Worker script name for domain router (deployed via wrangler, referenced here for route binding)"
  type        = string
  default     = "domain-router"
}
