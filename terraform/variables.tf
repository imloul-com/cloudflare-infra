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

variable "apex_proxy_ipv4" {
  description = "IPv4 address used for proxied apex A record. Traffic is served by Cloudflare proxy + Worker route."
  type        = string
  default     = "192.0.2.1"
}
