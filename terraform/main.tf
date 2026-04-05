data "cloudflare_zone" "root" {
  filter = {
    name = var.zone_name
  }
}

locals {
  dev_fqdn = "${var.dev_subdomain}.${var.zone_name}"
  managed_pages_projects = toset(concat(
    [
      "${var.portfolio_pages_project_name}${var.dev_project_suffix}",
      "${var.ast_viz_pages_project_name}${var.dev_project_suffix}"
    ],
    var.manage_prod_pages_projects ? [
      var.portfolio_pages_project_name,
      var.ast_viz_pages_project_name
    ] : []
  ))
}

resource "cloudflare_workers_script" "dev_router_bootstrap" {
  account_id  = var.account_id
  script_name = var.dev_router_worker_name
  content     = file("${path.module}/bootstrap-worker.js")
}

resource "cloudflare_pages_project" "managed" {
  for_each          = local.managed_pages_projects
  account_id        = var.account_id
  name              = each.key
  production_branch = "main"
}

resource "cloudflare_workers_route" "root_domain_route" {
  zone_id = data.cloudflare_zone.root.zone_id
  pattern = "${var.zone_name}/*"
  script  = var.router_worker_name
}

resource "cloudflare_workers_route" "dev_domain_route" {
  zone_id = data.cloudflare_zone.root.zone_id
  pattern = "${local.dev_fqdn}/*"
  script  = var.dev_router_worker_name
  depends_on = [
    cloudflare_workers_script.dev_router_bootstrap
  ]
}

resource "cloudflare_dns_record" "apex_a" {
  zone_id = data.cloudflare_zone.root.zone_id
  name    = var.zone_name
  type    = "A"
  content = var.apex_proxy_ipv4
  proxied = true
  ttl     = 1
}

resource "cloudflare_dns_record" "www_cname" {
  zone_id = data.cloudflare_zone.root.zone_id
  name    = "www"
  type    = "CNAME"
  content = var.zone_name
  proxied = true
  ttl     = 1
}

resource "cloudflare_dns_record" "dev_cname" {
  zone_id = data.cloudflare_zone.root.zone_id
  name    = var.dev_subdomain
  type    = "CNAME"
  content = var.zone_name
  proxied = true
  ttl     = 1
}

resource "cloudflare_ruleset" "global_rate_limit_all_pages" {
  zone_id = data.cloudflare_zone.root.zone_id
  name    = "Global rate limit - all pages"
  kind    = "zone"
  phase   = "http_ratelimit"

  rules = [{
    ref         = "rl_all_pages"
    description = "Protect Worker daily quota from abuse"
    expression  = "http.host in {\"${var.zone_name}\" \"${local.dev_fqdn}\"}"
    action      = "block"
    ratelimit = {
      characteristics     = ["ip.src", "cf.colo.id"]
      period              = 10
      requests_per_period = 50
      mitigation_timeout  = 10
    }
  }]
}
