output "portfolio_pages_project" {
  value = var.portfolio_project_name
}

output "ast_viz_pages_project" {
  value = var.ast_viz_project_name
}

output "portfolio_origin" {
  value = "https://${cloudflare_pages_domain.portfolio_domain.name}"
}

output "ast_viz_origin" {
  value = "https://${cloudflare_pages_domain.ast_viz_domain.name}"
}

output "resolved_portfolio_pages_subdomain" {
  value = local.portfolio_pages_subdomain
}

output "resolved_ast_viz_pages_subdomain" {
  value = local.ast_viz_pages_subdomain
}

output "router_script_name" {
  value = cloudflare_workers_script.domain_router.script_name
}

output "router_route_pattern" {
  value = cloudflare_workers_route.root_domain_route.pattern
}
