#!/usr/bin/env node

/**
 * Reads the version from Cargo.toml and updates all npm package.json files to match.
 */

import { readFileSync, writeFileSync } from "fs";
import { join, dirname } from "path";
import { fileURLToPath } from "url";

const __dirname = dirname(fileURLToPath(import.meta.url));
const root = join(__dirname, "..", "..");

// Read version from Cargo.toml
const cargoToml = readFileSync(join(root, "Cargo.toml"), "utf8");
const match = cargoToml.match(/^version\s*=\s*"([^"]+)"/m);
if (!match) {
  console.error("Could not find version in Cargo.toml");
  process.exit(1);
}
const version = match[1];
console.log(`Syncing npm packages to version ${version}`);

// All package.json files to update
const packages = [
  "npm/cli/package.json",
  "npm/cli-darwin-arm64/package.json",
  "npm/cli-darwin-x64/package.json",
  "npm/cli-linux-x64/package.json",
  "npm/cli-linux-arm64/package.json",
  "npm/cli-win32-x64/package.json",
];

for (const pkgPath of packages) {
  const fullPath = join(root, pkgPath);
  const pkg = JSON.parse(readFileSync(fullPath, "utf8"));
  pkg.version = version;

  // Update optionalDependencies versions in the root CLI package
  if (pkg.optionalDependencies) {
    for (const dep of Object.keys(pkg.optionalDependencies)) {
      pkg.optionalDependencies[dep] = version;
    }
  }

  writeFileSync(fullPath, JSON.stringify(pkg, null, 2) + "\n");
  console.log(`  Updated ${pkgPath}`);
}

console.log("Done.");
