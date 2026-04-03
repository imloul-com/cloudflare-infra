import type { Route, RouteMatch } from "./types";

export function matchRoute(pathname: string, routes: Route[]): RouteMatch | null {
  for (const route of routes) {
    if (route.prefix === "/") {
      const upstream = new URL(route.origin);
      upstream.pathname = pathname;
      return { route, upstream };
    }

    if (pathname === route.prefix || pathname.startsWith(route.prefix + "/")) {
      const upstream = new URL(route.origin);
      upstream.pathname = route.stripPrefix
        ? stripPrefix(pathname, route.prefix)
        : pathname;
      return { route, upstream };
    }
  }

  return null;
}

function stripPrefix(pathname: string, prefix: string): string {
  const stripped = pathname.slice(prefix.length);
  if (stripped === "" || stripped[0] !== "/") {
    return "/" + stripped;
  }
  return stripped;
}

export async function proxyRequest(
  request: Request,
  match: RouteMatch,
): Promise<Response> {
  const incoming = new URL(request.url);
  match.upstream.search = incoming.search;

  const upstreamRequest = new Request(match.upstream.toString(), request);
  const response = await fetch(upstreamRequest);

  if (!match.route.stripPrefix || match.route.prefix === "/") {
    return response;
  }

  const contentType = response.headers.get("content-type") ?? "";
  if (!contentType.includes("text/html")) {
    return response;
  }

  return injectBaseTag(response, match.route.prefix);
}

async function injectBaseTag(response: Response, prefix: string): Promise<Response> {
  const html = await response.text();
  const baseHref = prefix.endsWith("/") ? prefix : prefix + "/";
  const baseTag = `<base href="${baseHref}">`;

  const injected = html.replace(/<head([^>]*)>/i, `<head$1>${baseTag}`);

  const headers = new Headers(response.headers);
  headers.set("content-length", new TextEncoder().encode(injected).length.toString());

  return new Response(injected, {
    status: response.status,
    statusText: response.statusText,
    headers,
  });
}
