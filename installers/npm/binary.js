"use strict";

const os = require("os");
const { existsSync } = require("fs");
const { join, dirname } = require("path");
const { spawnSync } = require("child_process");

// Maps os.platform()-os.arch() to candidate optional package names.
// Linux x64 lists glibc first; npm installs exactly one based on the libc field,
// so only one will resolve at runtime.
const PLATFORM_PACKAGES = {
  "darwin-arm64": ["@apollo/rover-darwin-arm64"],
  "darwin-x64": ["@apollo/rover-darwin-x64"],
  "linux-arm64": ["@apollo/rover-linux-arm64"],
  "linux-x64": ["@apollo/rover-linux-x64", "@apollo/rover-linux-x64-musl"],
  "win32-x64": ["@apollo/rover-win32-x64"],
};

// Accepts optional deps for testing: platformKey, tryResolve, fileExists
const getBinaryPath = (deps = {}) => {
  const platformKey =
    deps.platformKey || `${os.platform()}-${os.arch()}`;
  const tryResolve =
    deps.tryResolve ||
    ((id) => {
      try {
        return require.resolve(id);
      } catch (_) {
        return null;
      }
    });
  const fileExists = deps.fileExists || existsSync;

  const candidates = PLATFORM_PACKAGES[platformKey];

  if (!candidates) {
    console.error(
      `Platform "${platformKey}" is not supported by rover.`,
    );
    console.error(
      `Supported platforms: ${Object.keys(PLATFORM_PACKAGES).join(", ")}`,
    );
    process.exit(1);
  }

  const ext = process.platform === "win32" ? ".exe" : "";

  for (const pkg of candidates) {
    const pkgJsonPath = tryResolve(`${pkg}/package.json`);
    if (!pkgJsonPath) continue;
    const bin = join(dirname(pkgJsonPath), "bin", `rover${ext}`);
    if (fileExists(bin)) return bin;
  }

  console.error(
    `Could not find rover binary for platform "${platformKey}".`,
  );
  console.error(`Try reinstalling @apollo/rover.`);
  process.exit(1);
};

const run = () => {
  const bin = getBinaryPath();

  // Allows Rust code to locate the directory for supergraph plugin extraction.
  process.env.APOLLO_NODE_MODULES_BIN_DIR = dirname(bin);

  const [, , ...args] = process.argv;
  const result = spawnSync(bin, args, { cwd: process.cwd(), stdio: "inherit" });

  if (result.error) {
    console.error(result.error);
    process.exit(1);
  }

  process.exit(result.status);
};

module.exports = { run, getBinaryPath };
