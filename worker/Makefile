.PHONY: test check dev deploy resolve-origins assemble-routes

ROUTE_DEFINITIONS_PATH ?= src/route-definitions.json

test:
	cargo test

check:
	cargo check

dev:
	npx wrangler dev

deploy:
	npx wrangler deploy

resolve-origins:
	@if [ -z "$(CLOUDFLARE_API_TOKEN)" ]; then echo "CLOUDFLARE_API_TOKEN is required"; exit 1; fi
	@if [ -z "$(CLOUDFLARE_ACCOUNT_ID)" ]; then echo "CLOUDFLARE_ACCOUNT_ID is required"; exit 1; fi
	@cargo run --quiet --bin resolve_origins -- --route-definitions-path "$(ROUTE_DEFINITIONS_PATH)"

assemble-routes:
	@command -v gh >/dev/null 2>&1 || { echo "gh CLI is required"; exit 1; }
	@command -v jq >/dev/null 2>&1 || { echo "jq is required"; exit 1; }
	@set -e; route_defs="[]"; \
	while IFS= read -r entry; do \
	  repo=$$(echo "$$entry" | jq -r '.repo'); \
	  pn=$$(gh api "repos/$${repo}/contents/wrangler.toml?ref=main" --jq '.content' \
	    | base64 -d | awk -F'"' '/^name[[:space:]]*=/ { print $$2; exit }'); \
	  if [ -z "$$pn" ]; then echo "Failed to read project name from $${repo}/wrangler.toml" >&2; exit 1; fi; \
	  route_def=$$(echo "$$entry" | jq --arg pn "$$pn" '. + {projectName: $$pn} | del(.repo)'); \
	  route_defs=$$(echo "$$route_defs" | jq --argjson r "$$route_def" '. + [$$r]'); \
	done < <(jq -c '.[]' src/app-sources.json); \
	echo "$$route_defs" | jq '.' > src/route-definitions.json
	@echo "Wrote src/route-definitions.json"
