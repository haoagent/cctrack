#!/usr/bin/env node
"use strict";

const path = require("path");
const { execFileSync } = require("child_process");
const os = require("os");

const binName = os.platform() === "win32" ? "cctrack.exe" : "cctrack";
const binPath = path.join(__dirname, binName);

try {
  execFileSync(binPath, process.argv.slice(2), { stdio: "inherit" });
} catch (err) {
  if (err.status != null) {
    process.exit(err.status);
  }
  console.error("Failed to run cctrack:", err.message);
  process.exit(1);
}
