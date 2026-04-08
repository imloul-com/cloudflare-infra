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

variable "dev_router_worker_name" {
  description = "Cloudflare Worker script name for dev domain router"
  type        = string
  default     = "domain-router-dev"
}

variable "dev_subdomain" {
  description = "Dev environment subdomain under the root zone"
  type        = string
  default     = "dev"
}

variable "portfolio_pages_project_name" {
  description = "Production Cloudflare Pages project name for portfolio site"
  type        = string
  default     = "portfolio"
}

variable "ast_viz_pages_project_name" {
  description = "Production Cloudflare Pages project name for ast-viz"
  type        = string
  default     = "ast-viz"
}

variable "bloom_filter_pages_project_name" {
  description = "Production Cloudflare Pages project name for bloom-filter"
  type        = string
  default     = "bloom-filter"
}

variable "dev_project_suffix" {
  description = "Suffix appended to production project names for dev projects"
  type        = string
  default     = "-dev"
}

variable "manage_prod_pages_projects" {
  description = "Whether Terraform should manage production Pages projects in addition to dev projects"
  type        = bool
  default     = true
}

variable "apex_proxy_ipv4" {
  description = "IPv4 address used for proxied apex A record. Traffic is served by Cloudflare proxy + Worker route."
  type        = string
  default     = "192.0.2.1"
}
