output "router_worker_name" {
  value = var.router_worker_name
}

output "router_route_pattern" {
  value = cloudflare_workers_route.root_domain_route.pattern
}

output "dev_router_worker_name" {
  value = var.dev_router_worker_name
}

output "dev_router_route_pattern" {
  value = cloudflare_workers_route.dev_domain_route.pattern
}

output "prod_domain" {
  value = var.zone_name
}

output "dev_domain" {
  value = "${var.dev_subdomain}.${var.zone_name}"
}

output "portfolio_pages_project_name" {
  value = var.portfolio_pages_project_name
}

output "portfolio_pages_project_name_dev" {
  value = "${var.portfolio_pages_project_name}${var.dev_project_suffix}"
}

output "ast_viz_pages_project_name" {
  value = var.ast_viz_pages_project_name
}

output "ast_viz_pages_project_name_dev" {
  value = "${var.ast_viz_pages_project_name}${var.dev_project_suffix}"
}
