export interface Route {
  prefix: string;
  origin: string;
  stripPrefix: boolean;
}

export interface RouteDefinition {
  prefix: string;
  stripPrefix: boolean;
  projectName: string;
  originVar: string;
}

export type Env = Record<string, string | undefined>;

export interface RouteMatch {
  route: Route;
  upstream: URL;
}
