import routeDefinitions from "./route-definitions.json";
import type { Env, Route, RouteDefinition } from "./types";

export function buildRoutes(env: Env): Route[] {
  const definitions = routeDefinitions as RouteDefinition[];

  return definitions.map((definition) => ({
    prefix: definition.prefix,
    stripPrefix: definition.stripPrefix,
    origin: getOriginOrMarker(env, definition.originVar),
  }));
}

function getOriginOrMarker(env: Env, key: string): string {
  const value = env[key];
  return value || `missing-binding:${key}`;
}
