import { describe, it, expect } from "vitest";
import { matchRoute } from "../src/router";
import type { Route } from "../src/types";

const TEST_ROUTES: Route[] = [
  {
    prefix: "/tools/ast-viz",
    origin: "https://worker-ast-viz.pages.dev",
    stripPrefix: true,
  },
  {
    prefix: "/",
    origin: "https://portfolio.pages.dev",
    stripPrefix: false,
  },
];

describe("matchRoute", () => {
  it("matches exact prefix", () => {
    const match = matchRoute("/tools/ast-viz", TEST_ROUTES);
    expect(match).not.toBeNull();
    expect(match!.route.prefix).toBe("/tools/ast-viz");
    expect(match!.upstream.pathname).toBe("/");
  });

  it("matches prefix with trailing slash", () => {
    const match = matchRoute("/tools/ast-viz/", TEST_ROUTES);
    expect(match).not.toBeNull();
    expect(match!.route.prefix).toBe("/tools/ast-viz");
    expect(match!.upstream.pathname).toBe("/");
  });

  it("matches prefix with nested path", () => {
    const match = matchRoute("/tools/ast-viz/grammar/code", TEST_ROUTES);
    expect(match).not.toBeNull();
    expect(match!.route.prefix).toBe("/tools/ast-viz");
    expect(match!.upstream.pathname).toBe("/grammar/code");
  });

  it("strips prefix correctly for assets", () => {
    const match = matchRoute("/tools/ast-viz/assets/index-abc123.js", TEST_ROUTES);
    expect(match).not.toBeNull();
    expect(match!.upstream.pathname).toBe("/assets/index-abc123.js");
  });

  it("does not match partial prefix", () => {
    const match = matchRoute("/tools/ast-vizzer", TEST_ROUTES);
    expect(match).not.toBeNull();
    expect(match!.route.prefix).toBe("/");
    expect(match!.upstream.pathname).toBe("/tools/ast-vizzer");
  });

  it("falls through to portfolio for root", () => {
    const match = matchRoute("/", TEST_ROUTES);
    expect(match).not.toBeNull();
    expect(match!.route.prefix).toBe("/");
    expect(match!.upstream.pathname).toBe("/");
  });

  it("falls through to portfolio for blog paths", () => {
    const match = matchRoute("/blog/my-post", TEST_ROUTES);
    expect(match).not.toBeNull();
    expect(match!.route.prefix).toBe("/");
    expect(match!.upstream.pathname).toBe("/blog/my-post");
  });

  it("returns null when no routes defined", () => {
    const match = matchRoute("/anything", []);
    expect(match).toBeNull();
  });

  it("preserves encoded path segments", () => {
    const match = matchRoute("/tools/ast-viz/path%20with%20spaces", TEST_ROUTES);
    expect(match).not.toBeNull();
    expect(match!.upstream.pathname).toBe("/path%20with%20spaces");
  });
});

describe("matchRoute ordering", () => {
  it("matches more specific prefix first", () => {
    const routes: Route[] = [
      {
        prefix: "/tools/ast-viz",
        origin: "https://ast.pages.dev",
        stripPrefix: true,
      },
      {
        prefix: "/tools",
        origin: "https://tools.pages.dev",
        stripPrefix: true,
      },
      {
        prefix: "/",
        origin: "https://root.pages.dev",
        stripPrefix: false,
      },
    ];

    const match = matchRoute("/tools/ast-viz/page", routes);
    expect(match!.route.origin).toBe("https://ast.pages.dev");

    const toolsMatch = matchRoute("/tools/other", routes);
    expect(toolsMatch!.route.origin).toBe("https://tools.pages.dev");
  });
});
