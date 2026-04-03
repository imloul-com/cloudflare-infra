import { describe, it, expect } from "vitest";
import worker from "../src/index";

const TEST_ENV = {
  PORTFOLIO_ORIGIN: "https://portfolio.pages.dev",
  AST_VIZ_ORIGIN: "https://worker-ast-viz.pages.dev",
};

describe("worker handler", () => {
  it("returns 200 on /_health", async () => {
    const response = await worker.fetch(
      new Request("https://imloul.com/_health"),
      TEST_ENV,
    );
    expect(response.status).toBe(200);
    expect(await response.text()).toBe("ok");
  });

  it("proxies root to portfolio origin", async () => {
    const response = await worker.fetch(new Request("https://imloul.com/"), TEST_ENV);
    expect(response.status).toBeGreaterThanOrEqual(200);
  });

  it("proxies /tools/ast-viz to ast-viz origin", async () => {
    const response = await worker.fetch(
      new Request("https://imloul.com/tools/ast-viz"),
      TEST_ENV,
    );
    expect(response.status).toBeGreaterThanOrEqual(200);
  });
});
