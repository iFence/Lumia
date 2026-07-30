#!/usr/bin/env node

import {
  generateKeyPairSync,
  sign,
  verify,
} from "node:crypto";
import { spawnSync } from "node:child_process";
import {
  cp,
  mkdir,
  mkdtemp,
  readFile,
  rm,
} from "node:fs/promises";
import path from "node:path";

const temporaryBase = path.resolve(process.env.RUNNER_TEMP ?? "target");
await mkdir(temporaryBase, { recursive: true });
const temporaryRoot = await mkdtemp(path.join(temporaryBase, "lumia-plugin-signing-"));
try {
  const installDirectory = "lumia-plugin-annotation";
  await cp(
    path.resolve("plugins", installDirectory),
    path.join(temporaryRoot, installDirectory),
    { recursive: true },
  );
  const { privateKey, publicKey } = generateKeyPairSync("ed25519");
  const publicDer = publicKey.export({ format: "der", type: "spki" });
  const expectedPublicKey = publicDer.subarray(publicDer.length - 32).toString("hex");
  const privatePem = privateKey.export({ format: "pem", type: "pkcs8" }).toString();
  const result = spawnSync(
    process.execPath,
    [
      "scripts/sign-plugin-package.mjs",
      "--root",
      temporaryRoot,
      "--install-directory",
      installDirectory,
      "--plugin-id",
      "lumia.annotation",
      "--target-os",
      "linux",
      "--target-arch",
      "x64",
      "--minimum-lumia-version",
      "0.1.2",
      "--plugin-api-version",
      "2",
      "--expected-public-key",
      expectedPublicKey,
    ],
    {
      cwd: process.cwd(),
      encoding: "utf8",
      env: {
        ...process.env,
        LUMIA_PLUGIN_SIGNING_KEY_PEM: privatePem,
      },
    },
  );
  if (result.status !== 0) {
    throw new Error(result.stderr || "signer exited unsuccessfully");
  }

  const manifestBytes = await readFile(path.join(temporaryRoot, "lumia.package.json"));
  const signature = Buffer.from(
    (await readFile(path.join(temporaryRoot, "lumia.package.sig"), "utf8")).trim(),
    "base64",
  );
  if (!verify(null, manifestBytes, publicKey, signature)) {
    throw new Error("generated package signature did not verify");
  }
  const manifest = JSON.parse(manifestBytes);
  if (
    manifest.target_arch !== "x86_64" ||
    manifest.plugin_id !== "lumia.annotation" ||
    !manifest.files.some((file) => file.path.endsWith("/lumia.plugin.json"))
  ) {
    throw new Error("generated package manifest is incomplete or not normalized");
  }
  const tampered = Buffer.concat([manifestBytes, Buffer.from(" ")]);
  if (verify(null, tampered, publicKey, signature)) {
    throw new Error("signature unexpectedly accepted tampered metadata");
  }
  const unrelatedSignature = sign(null, manifestBytes, privateKey);
  if (!unrelatedSignature.equals(signature)) {
    throw new Error("Ed25519 signing should be deterministic");
  }

  const rawRoot = path.join(temporaryRoot, "raw-fixture");
  const rawInstallDirectory = "lumia-plugin-raw";
  const rawPluginRoot = path.join(rawRoot, rawInstallDirectory);
  await mkdir(rawPluginRoot, { recursive: true });
  await cp(
    path.resolve("plugins", rawInstallDirectory, "lumia.plugin.json"),
    path.join(rawPluginRoot, "lumia.plugin.json"),
  );
  const rawResult = spawnSync(
    process.execPath,
    [
      "scripts/sign-plugin-package.mjs",
      "--root",
      rawRoot,
      "--install-directory",
      rawInstallDirectory,
      "--plugin-id",
      "lumia.raw",
      "--target-os",
      "windows",
      "--target-arch",
      "x64",
      "--minimum-lumia-version",
      "0.1.5",
      "--plugin-api-version",
      "2",
      "--expected-public-key",
      expectedPublicKey,
    ],
    {
      cwd: process.cwd(),
      encoding: "utf8",
      env: {
        ...process.env,
        LUMIA_PLUGIN_SIGNING_KEY_PEM: privatePem,
      },
    },
  );
  if (rawResult.status !== 0) {
    throw new Error(rawResult.stderr || "RAW signer exited unsuccessfully");
  }
  const rawPackage = JSON.parse(
    await readFile(path.join(rawRoot, "lumia.package.json")),
  );
  const rawManifestBytes = await readFile(
    path.join(rawPluginRoot, "lumia.plugin.json"),
  );
  const rawRuntimeSignature = Buffer.from(
    (await readFile(path.join(rawPluginRoot, "lumia.plugin.sig"), "utf8")).trim(),
    "base64",
  );
  if (
    rawPackage.plugin_id !== "lumia.raw" ||
    !verify(null, rawManifestBytes, publicKey, rawRuntimeSignature)
  ) {
    throw new Error("RAW runtime manifest was not signed correctly");
  }
  process.stdout.write("Plugin package signer fixture passed\n");
} finally {
  await rm(temporaryRoot, { recursive: true, force: true });
}
