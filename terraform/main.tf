data "cloudflare_zone" "root" {
  filter = {
    name = var.zone_name
  }
}

data "cloudflare_pages_projects" "all" {
  account_id = var.account_id
}

locals {
  # Resolve real Pages hostnames from your Cloudflare account.
  portfolio_pages_subdomain = one([
    for project in data.cloudflare_pages_projects.all.result : project.subdomain
    if project.name == var.portfolio_project_name
  ])
  ast_viz_pages_subdomain = one([
    for project in data.cloudflare_pages_projects.all.result : project.subdomain
    if project.name == var.ast_viz_project_name
  ])

  portfolio_origin = "https://${local.portfolio_pages_subdomain}"
  ast_viz_origin   = "https://${local.ast_viz_pages_subdomain}"
}

resource "cloudflare_workers_script" "domain_router" {
  account_id  = var.account_id
  script_name = var.router_worker_name
  content = templatefile("${path.module}/../worker/router.js.tmpl", {
    ast_viz_prefix   = var.ast_viz_mount_prefix
    ast_viz_origin   = local.ast_viz_origin
    portfolio_origin = local.portfolio_origin
  })
}

resource "cloudflare_workers_route" "root_domain_route" {
  zone_id = data.cloudflare_zone.root.zone_id
  pattern = "${var.zone_name}/*"
  script  = cloudflare_workers_script.domain_router.script_name
}
