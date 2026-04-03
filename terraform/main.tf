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
