#!/usr/bin/env node
"use strict";

const os = require("os");
const fs = require("fs");
const path = require("path");
const { execSync } = require("child_process");
const https = require("https");

const VERSION = require("./package.json").version;
const REPO = "haoagent/cctrack";

const PLATFORM_MAP = {
  "darwin-arm64": "aarch64-apple-darwin",
  "darwin-x64": "x86_64-apple-darwin",
  "linux-x64": "x86_64-unknown-linux-gnu",
  "linux-arm64": "aarch64-unknown-linux-gnu",
  "win32-x64": "x86_64-pc-windows-msvc",
};

function getTarget() {
  const platform = os.platform();
  const arch = os.arch();
  const key = `${platform}-${arch}`;
  const target = PLATFORM_MAP[key];
  if (!target) {
    console.error(`Unsupported platform: ${platform}-${arch}`);
    console.error(`Supported: ${Object.keys(PLATFORM_MAP).join(", ")}`);
    process.exit(1);
  }
  return target;
}

function follow(url) {
  return new Promise((resolve, reject) => {
    https.get(url, { headers: { "User-Agent": "cctrack-npm" } }, (res) => {
      if (res.statusCode >= 300 && res.statusCode < 400 && res.headers.location) {
        follow(res.headers.location).then(resolve, reject);
      } else if (res.statusCode !== 200) {
        reject(new Error(`HTTP ${res.statusCode} for ${url}`));
      } else {
        const chunks = [];
        res.on("data", (c) => chunks.push(c));
        res.on("end", () => resolve(Buffer.concat(chunks)));
        res.on("error", reject);
      }
    }).on("error", reject);
  });
}

async function install() {
  const target = getTarget();
  const isWindows = os.platform() === "win32";
  const ext = isWindows ? "zip" : "tar.gz";
  const asset = `cctrack-${target}.${ext}`;
  const url = `https://github.com/${REPO}/releases/download/v${VERSION}/${asset}`;

  const binDir = path.join(__dirname, "bin");
  const binName = isWindows ? "cctrack.exe" : "cctrack";
  const binPath = path.join(binDir, binName);

  // Skip if already installed
  if (fs.existsSync(binPath)) {
    return;
  }

  console.log(`Downloading cctrack v${VERSION} for ${target}...`);

  const data = await follow(url);
  const tmpFile = path.join(binDir, asset);
  fs.mkdirSync(binDir, { recursive: true });
  fs.writeFileSync(tmpFile, data);

  if (isWindows) {
    // Use PowerShell to extract zip
    execSync(
      `powershell -Command "Expand-Archive -Path '${tmpFile}' -DestinationPath '${binDir}' -Force"`,
      { stdio: "ignore" }
    );
  } else {
    execSync(`tar xzf "${tmpFile}" -C "${binDir}"`, { stdio: "ignore" });
    fs.chmodSync(binPath, 0o755);
  }

  // Clean up archive
  fs.unlinkSync(tmpFile);
  console.log(`Installed cctrack to ${binPath}`);
}

install().catch((err) => {
  console.error("Failed to install cctrack:", err.message);
  console.error("You can install manually from https://github.com/haoagent/cctrack/releases");
  process.exit(1);
});
