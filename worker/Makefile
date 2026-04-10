.PHONY: test check validate-catalog dev deploy resolve-origins assemble-routes bump-version diff-versions

ROUTE_DEFINITIONS_PATH ?= src/route-definitions.json
ENVIRONMENT ?= prod
APP ?=
VERSION ?=
APPS_YAML ?= apps.yaml
OLD_APPS_YAML ?=
NEW_APPS_YAML ?=

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

bump-version:
	@if [ -z "$(APP)" ]; then echo "APP is required (e.g. APP=ast-viz)"; exit 1; fi
	@if [ -z "$(ENVIRONMENT)" ]; then echo "ENVIRONMENT is required (prod|dev)"; exit 1; fi
	@if [ -z "$(VERSION)" ]; then echo "VERSION is required (e.g. VERSION=sha-abc123def456)"; exit 1; fi
	@cargo run --quiet --bin bump_version -- --app "$(APP)" --env "$(ENVIRONMENT)" --version "$(VERSION)" --apps-yaml "$(APPS_YAML)"

diff-versions:
	@if [ -z "$(OLD_APPS_YAML)" ]; then echo "OLD_APPS_YAML is required"; exit 1; fi
	@if [ -z "$(NEW_APPS_YAML)" ]; then echo "NEW_APPS_YAML is required"; exit 1; fi
	@cargo run --quiet --bin diff_versions -- --old "$(OLD_APPS_YAML)" --new "$(NEW_APPS_YAML)"
