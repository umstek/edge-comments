import { execFileSync } from "node:child_process";
import { copyFileSync, existsSync, mkdirSync, rmSync, writeFileSync } from "node:fs";
import path from "node:path";

const rawArgs = process.argv.slice(2);
const skipBuild = rawArgs.includes("--skip-build");
const dryRun = rawArgs.includes("--dry-run");

let mode = "production";
for (let i = 0; i < rawArgs.length; i++) {
  if (rawArgs[i] === "--mode") {
    mode = rawArgs[i + 1] ?? "";
    i++;
  } else if (rawArgs[i].startsWith("--mode=")) {
    mode = rawArgs[i].slice("--mode=".length);
  }
}

const MODES = {
  production: {
    workerName: "edge-comments",
    dbName: "edge-comments",
    domains: ["comments.umstek.com"],
  },
  staging: {
    workerName: "edge-comments-staging",
    dbName: "edge-comments-staging",
    domains: ["comments.staging.umstek.com"],
  },
};

const modeConfig = MODES[mode];
if (!modeConfig) {
  console.error(`Unknown --mode ${JSON.stringify(mode)} (expected "production" or "staging").`);
  process.exit(1);
}

const root = path.resolve(import.meta.dirname, "..");
const buildDir = path.join(root, "build");
const outDir = path.join(root, ".cloudflare", "output", "v1");
const workerDir = path.join(outDir, "workers", "default");
const bundleDir = path.join(workerDir, "bundle");

if (!skipBuild) {
  execFileSync("worker-build", ["--release"], { cwd: root, stdio: "inherit" });
}

const mainJs = path.join(buildDir, "index.js");
const mainWasm = path.join(buildDir, "index_bg.wasm");
if (!existsSync(mainJs) || !existsSync(mainWasm)) {
  console.error("Missing worker-build artifacts in build/ (expected index.js and index_bg.wasm).");
  process.exit(1);
}

const envD1Id = process.env.CF_D1_ID?.trim();
const d1Id = envD1Id || (dryRun ? "00000000-0000-0000-0000-000000000000" : "");
if (!d1Id) {
  console.error("CF_D1_ID is required (pass --dry-run to validate with a placeholder UUID).");
  process.exit(1);
}

const env = {
  DB: {
    type: "d1",
    name: modeConfig.dbName,
    id: d1Id,
  },
};

const allowedOrigins = process.env.ALLOWED_ORIGINS?.trim();
if (allowedOrigins) {
  env.ALLOWED_ORIGINS = { type: "text", value: allowedOrigins };
}

const allowedSites = process.env.ALLOWED_SITES?.trim();
if (allowedSites) {
  env.ALLOWED_SITES = { type: "text", value: allowedSites };
}

rmSync(path.join(root, ".cloudflare", "output"), { recursive: true, force: true });
mkdirSync(bundleDir, { recursive: true });

writeFileSync(
  path.join(outDir, "config.json"),
  `${JSON.stringify(
    {
      producer: { name: "edge-comments-build", version: "0.3.0" },
      buildContext: { isPreview: false, mode },
    },
    null,
    2,
  )}\n`,
);

writeFileSync(
  path.join(workerDir, "worker.config.json"),
  `${JSON.stringify(
    {
      name: modeConfig.workerName,
      compatibilityDate: "2026-07-01",
      domains: modeConfig.domains,
      workersDev: true,
      observability: { enabled: true },
      env,
      manifest: {
        type: "complete",
        mainModule: "index.js",
        modules: {
          "index.js": { type: "esm" },
          "index_bg.wasm": { type: "wasm" },
        },
      },
    },
    null,
    2,
  )}\n`,
);

copyFileSync(mainJs, path.join(bundleDir, "index.js"));
copyFileSync(mainWasm, path.join(bundleDir, "index_bg.wasm"));
console.log(`Built cf output (${mode}) at ${path.relative(root, outDir)}`);
