.PHONY: test check validate-catalog dev deploy resolve-origins assemble-routes

ROUTE_DEFINITIONS_PATH ?= src/route-definitions.json
ENVIRONMENT ?= prod

test:
	cargo test

check:
	cargo check

validate-catalog:
	@cargo run --quiet --bin validate_catalog

dev:
	npx wrangler dev

deploy:
	npx wrangler deploy

resolve-origins:
	@if [ -z "$(CLOUDFLARE_API_TOKEN)" ]; then echo "CLOUDFLARE_API_TOKEN is required"; exit 1; fi
	@if [ -z "$(CLOUDFLARE_ACCOUNT_ID)" ]; then echo "CLOUDFLARE_ACCOUNT_ID is required"; exit 1; fi
	@cargo run --quiet --bin resolve_origins -- --route-definitions-path "$(ROUTE_DEFINITIONS_PATH)"

assemble-routes:
	@cargo run --quiet --bin assemble_routes -- --environment "$(ENVIRONMENT)"
