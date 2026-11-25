import { execFileSync } from "node:child_process";
import { existsSync, readdirSync, readFileSync } from "node:fs";
import path from "node:path";
import { Miniflare } from "miniflare";

const args = new Set(process.argv.slice(2));
const root = path.resolve(import.meta.dirname, "..");
const buildDir = path.join(root, "build");
const mainJs = path.join(buildDir, "index.js");
const mainWasm = path.join(buildDir, "index_bg.wasm");

if (args.has("--build") || !existsSync(mainJs) || !existsSync(mainWasm)) {
  execFileSync("worker-build", ["--release"], { cwd: root, stdio: "inherit" });
}

const env = {
  DB: { type: "d1", name: "edge-comments", id: "edge-comments" },
  ALLOW_INSECURE_WRITES: {
    type: "text",
    value: process.env.ALLOW_INSECURE_WRITES ?? "true",
  },
};

for (const key of ["ALLOWED_ORIGINS", "ALLOWED_SITES", "TURNSTILE_SECRET", "ADMIN_TOKEN"]) {
  const val = process.env[key];
  if (val !== undefined && val !== "") {
    env[key] = { type: "text", value: val };
  }
}

const mf = new Miniflare({
  host: "127.0.0.1",
  port: Number(process.env.PORT ?? 8787),
  resourcePersistencePath: path.join(root, ".cloudflare", "state", "v3"),
  workers: [
    {
      config: {
        name: "edge-comments",
        compatibilityDate: "2026-07-01",
        manifest: {
          mainModule: "index.js",
          modulesRoot: buildDir,
          modules: {
            "index.js": { type: "esm", contents: readFileSync(mainJs, "utf8") },
            "index_bg.wasm": { type: "wasm", contents: readFileSync(mainWasm) },
          },
        },
        env,
      },
    },
  ],
});

const db = await mf.getD1Database("DB");
const migrationsDir = path.join(root, "migrations");
if (existsSync(migrationsDir)) {
  const files = readdirSync(migrationsDir)
    .filter((f) => f.endsWith(".sql"))
    .sort();
  for (const file of files) {
    const raw = readFileSync(path.join(migrationsDir, file), "utf8");
    const statements = raw
      .split(/\r?\n/)
      .filter((line) => !line.trimStart().startsWith("--"))
      .join("\n")
      .split(";")
      .map((s) => s.trim())
      .filter(Boolean);
    if (statements.length > 0) {
      await db.batch(statements.map((stmt) => db.prepare(stmt)));
    }
  }
}

const url = await mf.ready;
console.log(`edge-comments local dev server listening at ${url.origin}`);

for (const signal of ["SIGINT", "SIGTERM"]) {
  process.on(signal, async () => {
    await mf.dispose();
    process.exit(0);
  });
}
