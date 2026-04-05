data "cloudflare_zone" "root" {
  filter = {
    name = var.zone_name
  }
}

resource "cloudflare_workers_route" "root_domain_route" {
  zone_id = data.cloudflare_zone.root.zone_id
  pattern = "${var.zone_name}/*"
  script  = var.router_worker_name
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

resource "cloudflare_ruleset" "global_rate_limit_all_pages" {
  zone_id = data.cloudflare_zone.root.zone_id
  name    = "Global rate limit - all pages"
  kind    = "zone"
  phase   = "http_ratelimit"

  rules = [{
    ref         = "rl_all_pages"
    description = "Protect Worker daily quota from abuse"
    expression  = "http.host eq \"${var.zone_name}\""
    action      = "block"
    ratelimit = {
      characteristics     = ["ip.src", "cf.colo.id"]
      period              = 10
      requests_per_period = 50
      mitigation_timeout  = 10
    }
  }]
}
