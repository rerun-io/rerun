import { defineConfig } from "@playwright/test";

const PORT = 5173;

export default defineConfig({
  testDir: "tests",
  timeout: 180_000,
  use: {
    baseURL: `http://localhost:${PORT}`,
    launchOptions: {
      // CI has Chrome preinstalled; locally, run `npx playwright install chromium` instead.
      executablePath: process.env.CHROME,
      // Machines without a GPU need the software rasterizer.
      args: ["--use-angle=swiftshader", "--enable-unsafe-swiftshader"],
    },
  },
  webServer: {
    command: `npx vite --port ${PORT} --strictPort`,
    url: `http://localhost:${PORT}`,
    reuseExistingServer: !process.env.CI,
  },
});
