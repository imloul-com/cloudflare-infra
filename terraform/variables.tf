variable "account_id" {
  description = "Cloudflare account ID"
  type        = string
}

variable "zone_name" {
  description = "Root zone managed in Cloudflare (example: imloul.com)"
  type        = string
}

variable "portfolio_project_name" {
  description = "Pages project name for portfolio app"
  type        = string
  default     = "portfolio"
}

variable "ast_viz_project_name" {
  description = "Pages project name for ast-viz app"
  type        = string
  default     = "worker-ast-viz"
}

variable "portfolio_subdomain" {
  description = "Subdomain used as stable upstream for portfolio"
  type        = string
  default     = "portfolio"
}

variable "ast_viz_subdomain" {
  description = "Subdomain used as stable upstream for ast-viz"
  type        = string
  default     = "ast-viz"
}

variable "ast_viz_mount_prefix" {
  description = "Path prefix mounted for ast-viz on the root domain"
  type        = string
  default     = "/tools/ast-viz"
}

variable "router_worker_name" {
  description = "Cloudflare Worker script name for domain router"
  type        = string
  default     = "domain-router"
}
