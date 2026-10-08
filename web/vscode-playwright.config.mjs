import { defineConfig } from "@playwright/test";
import config from "./playwright.config.mjs";
export default defineConfig({ ...config, testDir: "../editors/tests/browser" });
