import { readFileSync, writeFileSync } from "node:fs";
const version = process.argv[2];
if (!/^\d+\.\d+\.\d+$/.test(version ?? "")) throw new Error("Supply a semantic application version.");
const check = process.argv.includes("--check");
for (const path of ["package.json", "package-lock.json", "src-tauri/tauri.conf.json"]) {
  const value = JSON.parse(readFileSync(path, "utf8"));
  if (check && (value.version !== version || (path === "package-lock.json" && value.packages[""].version !== version))) throw new Error(`Version mismatch: ${path}`);
  value.version = version;
  if (path === "package-lock.json") value.packages[""].version = version;
  if (!check) writeFileSync(path, JSON.stringify(value, null, 2) + "\n");
}
for (const path of ["src-tauri/Cargo.toml", "src-tauri/Cargo.lock"]) {
  const source = readFileSync(path, "utf8");
  const pattern = /(name = "screenshot-renamer"\r?\nversion = ")([^"]+)(")/g;
  const matches = [...source.matchAll(pattern)];
  if (matches.length !== 1 || (check && matches[0][2] !== version)) throw new Error(`Version mismatch: ${path}`);
  if (!check) writeFileSync(path, source.replace(pattern, (_, before, old, after) => before + version + after));
}
console.log(`Application version ${version} ${check ? "verified" : "saved"}.`);
