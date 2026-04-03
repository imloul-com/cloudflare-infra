import type { Env } from "./types";
import { buildRoutes } from "./routes";
import { matchRoute, proxyRequest } from "./router";

export default {
  async fetch(request: Request, env: Env): Promise<Response> {
    const url = new URL(request.url);
    const start = Date.now();
    let matchedPrefix: string | undefined;
    let upstreamUrl: string | undefined;

    if (url.pathname === "/_health") {
      return new Response("ok", { status: 200 });
    }

    try {
      const routes = buildRoutes(env);
      const match = matchRoute(url.pathname, routes);

      if (!match) {
        console.error(
          JSON.stringify({
            event: "no_route_match",
            pathname: url.pathname,
          }),
        );
        return new Response("No matching route", {
          status: 502,
          headers: { "x-router-error": "no_match" },
        });
      }

      if (match.route.origin.startsWith("missing-binding:")) {
        const missingBinding = match.route.origin.replace("missing-binding:", "");
        console.error(
          JSON.stringify({
            event: "route_misconfigured",
            pathname: url.pathname,
            matched_prefix: match.route.prefix,
            missing_binding: missingBinding,
          }),
        );
        return new Response("Route is misconfigured", {
          status: 503,
          headers: {
            "x-router-error": "missing_binding",
            "x-router-missing-binding": missingBinding,
          },
        });
      }

      matchedPrefix = match.route.prefix;
      upstreamUrl = match.upstream.toString();
      const response = await proxyRequest(request, match);
      const elapsed = Date.now() - start;

      console.log(
        JSON.stringify({
          event: "request",
          method: request.method,
          pathname: url.pathname,
          matched_prefix: match.route.prefix,
          upstream: match.upstream.origin,
          upstream_path: match.upstream.pathname,
          status: response.status,
          duration_ms: elapsed,
        }),
      );

      return response;
    } catch (err) {
      const elapsed = Date.now() - start;
      const message = err instanceof Error ? err.message : String(err);

      console.error(
        JSON.stringify({
          event: "proxy_error",
          pathname: url.pathname,
          matched_prefix: matchedPrefix,
          upstream: upstreamUrl,
          error: message,
          duration_ms: elapsed,
        }),
      );

      return new Response("Upstream error", {
        status: 502,
        headers: { "x-router-error": "upstream_failure" },
      });
    }
  },
} satisfies ExportedHandler<Env>;
