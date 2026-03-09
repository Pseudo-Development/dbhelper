import { defineConfig } from "drizzle-kit";

// This config is not used directly — migrations are generated via CLI flags.
// See package.json scripts for the actual invocations.
export default defineConfig({
  dialect: "postgresql",
  schema: "./src/schema/postgres.ts",
  out: "./migrations/postgres",
});
