output "router_worker_name" {
  value = var.router_worker_name
}

output "router_route_pattern" {
  value = cloudflare_workers_route.root_domain_route.pattern
}
