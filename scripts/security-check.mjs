#!/usr/bin/env node

import { spawnSync } from "node:child_process";
import { existsSync, readdirSync, readFileSync } from "node:fs";
import { join, relative } from "node:path";
import { cwd, exit } from "node:process";

const root = cwd();
const includeBuild = process.argv.includes("--full") || process.argv.includes("--with-build");

const moderateAuditPackages = [
  "bindings/typescript",
  "bindings/typescript-spark",
  "bindings/typescript-arkade",
  "bindings/typescript-spark/examples/spark-web",
  "bindings/lni_nodejs",
];

const highAuditPackages = [
  "bindings/typescript-spark/examples/spark-expo-go",
  "bindings/lni_react_native",
];

const buildChecks = [
  ["npm", ["--prefix", "bindings/typescript", "run", "typecheck"]],
  ["npm", ["--prefix", "bindings/typescript-spark", "run", "typecheck"]],
  ["npm", ["--prefix", "bindings/typescript-arkade", "run", "typecheck"]],
  ["npm", ["--prefix", "bindings/typescript", "run", "pack:dry-run"]],
];

function heading(text) {
  console.log(`\n==> ${text}`);
}

function run(command, args) {
  console.log(`$ ${command} ${args.join(" ")}`);
  const result = spawnSync(command, args, {
    cwd: root,
    stdio: "inherit",
    shell: false,
  });

  if (result.error) {
    console.error(result.error.message);
    return false;
  }

  return result.status === 0;
}

const severityRank = {
  info: 0,
  low: 1,
  moderate: 2,
  high: 3,
  critical: 4,
};

function runAudit(packagePath, threshold) {
  const thresholdRank = severityRank[threshold];
  const args = ["--prefix", packagePath, "audit", "--json"];

  console.log(`$ npm ${args.join(" ")} (${threshold}+ threshold)`);

  const result = spawnSync("npm", args, {
    cwd: root,
    encoding: "utf8",
    shell: false,
  });

  const output = result.stdout || result.stderr;
  if (!output) {
    console.error("npm audit produced no output.");
    return false;
  }

  let audit;
  try {
    audit = JSON.parse(output);
  } catch {
    console.error("Unable to parse npm audit JSON output.");
    console.error(output);
    return false;
  }

  if (audit.error) {
    console.error(audit.error.summary ?? audit.error.message ?? "npm audit failed.");
    if (audit.error.detail) console.error(audit.error.detail);
    return false;
  }

  const vulnerabilities = Object.entries(audit.vulnerabilities ?? {});
  const failing = vulnerabilities.filter(([, vulnerability]) => {
    return (severityRank[vulnerability.severity] ?? 0) >= thresholdRank;
  });

  if (failing.length > 0) {
    console.error(`Found ${failing.length} ${threshold}+ vulnerability group(s) in ${packagePath}:`);
    for (const [name, vulnerability] of failing) {
      console.error(`  - ${name}: ${vulnerability.severity}`);
    }
    return false;
  }

  const ignored = vulnerabilities.filter(([, vulnerability]) => {
    const rank = severityRank[vulnerability.severity] ?? 0;
    return rank > 0 && rank < thresholdRank;
  });

  if (ignored.length > 0) {
    console.log(`passed; ignored ${ignored.length} vulnerability group(s) below ${threshold}`);
  } else {
    console.log("passed; no vulnerabilities at or above threshold");
  }

  return true;
}

function versionParts(version) {
  const parts = String(version)
    .split(/[.-]/)
    .map((part) => (/^\d+$/.test(part) ? Number(part) : null))
    .filter((part) => part !== null);

  while (parts.length < 3) {
    parts.push(0);
  }

  return parts.slice(0, 3);
}

function compareVersions(left, right) {
  const a = versionParts(left);
  const b = versionParts(right);

  for (let index = 0; index < 3; index += 1) {
    if (a[index] < b[index]) return -1;
    if (a[index] > b[index]) return 1;
  }

  return 0;
}

function lt(version, target) {
  return compareVersions(version, target) < 0;
}

function ge(version, target) {
  return compareVersions(version, target) >= 0;
}

function major(version) {
  return versionParts(version)[0];
}

function isReportVulnerable(name, version) {
  const m = major(version);

  switch (name) {
    case "brace-expansion":
      return (
        (m === 1 && lt(version, "1.1.13")) ||
        (m === 2 && lt(version, "2.0.3")) ||
        (m === 3 && lt(version, "3.0.2")) ||
        (m === 4 && lt(version, "4.0.1")) ||
        (m === 5 && lt(version, "5.0.5"))
      );
    case "lodash":
      return lt(version, "4.18.0");
    case "js-yaml":
      return (m === 3 && lt(version, "3.14.2")) || (m === 4 && lt(version, "4.1.1"));
    case "ajv":
      return (m === 6 && lt(version, "6.14.0")) || m === 7 || (m === 8 && lt(version, "8.18.0"));
    case "minimatch":
      return (
        (m === 3 && lt(version, "3.1.4")) ||
        (m === 5 && lt(version, "5.1.8")) ||
        (m === 8 && lt(version, "8.0.6")) ||
        (m === 9 && lt(version, "9.0.7")) ||
        (m === 10 && lt(version, "10.2.3"))
      );
    case "picomatch":
      return (
        (m === 2 && lt(version, "2.3.2")) ||
        (m === 3 && lt(version, "3.0.2")) ||
        (m === 4 && lt(version, "4.0.4"))
      );
    case "esbuild":
      return lt(version, "0.25.0");
    case "glob":
      return (m === 10 && ge(version, "10.2.0") && lt(version, "10.5.0")) || (m === 11 && lt(version, "11.1.0"));
    case "tar":
      return lt(version, "7.5.11");
    case "vite":
      return m < 6 || (m === 6 && lt(version, "6.4.2")) || (m === 7 && lt(version, "7.3.2")) || (m === 8 && lt(version, "8.0.5"));
    case "yaml":
      return (m === 1 && lt(version, "1.10.3")) || (m === 2 && lt(version, "2.8.3"));
    case "rollup":
      return m < 2 || (m === 2 && lt(version, "2.80.0")) || (m === 3 && lt(version, "3.30.0")) || (m === 4 && lt(version, "4.59.0"));
    case "left-pad":
    case "es5-ext":
      return true;
    default:
      return false;
  }
}

function packageNameFromLockPath(lockPath) {
  const marker = "node_modules/";
  const index = lockPath.lastIndexOf(marker);
  if (index === -1) return null;

  const packagePath = lockPath.slice(index + marker.length);
  const parts = packagePath.split("/");

  if (parts[0]?.startsWith("@") && parts.length >= 2) {
    return `${parts[0]}/${parts[1]}`;
  }

  return parts[0] ?? null;
}

function findPackageLocks(directory) {
  const entries = readdirSync(directory, { withFileTypes: true });
  const locks = [];

  for (const entry of entries) {
    if (entry.name === "node_modules" || entry.name === ".git" || entry.name === "target" || entry.name === ".build") {
      continue;
    }

    const path = join(directory, entry.name);
    if (entry.isDirectory()) {
      locks.push(...findPackageLocks(path));
    } else if (entry.isFile() && entry.name === "package-lock.json") {
      locks.push(path);
    }
  }

  return locks;
}

function checkSocketReportRanges() {
  const watchedPackages = new Set([
    "brace-expansion",
    "lodash",
    "js-yaml",
    "ajv",
    "minimatch",
    "picomatch",
    "esbuild",
    "glob",
    "tar",
    "vite",
    "yaml",
    "rollup",
    "left-pad",
    "es5-ext",
  ]);

  const findings = [];

  for (const lockPath of findPackageLocks(root)) {
    const lock = JSON.parse(readFileSync(lockPath, "utf8"));

    for (const [path, metadata] of Object.entries(lock.packages ?? {})) {
      const name = packageNameFromLockPath(path);
      const version = metadata?.version;

      if (!name || !version || !watchedPackages.has(name)) continue;
      if (isReportVulnerable(name, version)) {
        findings.push(`${relative(root, lockPath)}: ${path} ${name}@${version}`);
      }
    }
  }

  if (findings.length > 0) {
    console.error("Found package-lock entries in vulnerable ranges from the Socket report:");
    for (const finding of findings) {
      console.error(`  - ${finding}`);
    }
    return false;
  }

  console.log("No package-lock entries match the vulnerable ranges from the Socket report.");
  return true;
}

let ok = true;

heading("Socket Report Range Check");
ok = checkSocketReportRanges() && ok;

heading("NPM Audits: Moderate Threshold");
for (const packagePath of moderateAuditPackages) {
  if (!existsSync(join(root, packagePath, "package-lock.json"))) {
    console.log(`Skipping ${packagePath}; no package-lock.json found.`);
    continue;
  }

  ok = runAudit(packagePath, "moderate") && ok;
}

heading("NPM Audits: High Threshold For Framework Examples");
for (const packagePath of highAuditPackages) {
  if (!existsSync(join(root, packagePath, "package-lock.json"))) {
    console.log(`Skipping ${packagePath}; no package-lock.json found.`);
    continue;
  }

  ok = runAudit(packagePath, "high") && ok;
}

if (includeBuild) {
  heading("Build-Oriented Validation");
  for (const [command, args] of buildChecks) {
    ok = run(command, args) && ok;
  }
} else {
  console.log("\nTip: run `node scripts/security-check.mjs --full` to include typechecks and pack:dry-run.");
}

if (!ok) {
  console.error("\nSecurity checks failed.");
  exit(1);
}

console.log("\nSecurity checks passed.");
