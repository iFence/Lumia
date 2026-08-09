#!/usr/bin/env node

import {
  createHash,
  createPrivateKey,
  createPublicKey,
  sign,
  verify,
} from "node:crypto";
import { lstat, readFile, readdir, writeFile } from "node:fs/promises";
import path from "node:path";
import { pathToFileURL } from "node:url";

export const OFFICIAL_PUBLIC_KEY_HEX =
  "6b88de1c86a73ae666d4a44b54e3046900ff24a085a2515ada36d2b15cc55417";
const OFFICIAL_PLUGIN_IDS = new Set(["lumia.annotation", "lumia.raw"]);

/// Community-index metadata for the official plugins. `lumia.plugin.json`
/// carries id/name/version/permissions only, so the fields a search needs
/// (description, tags, author, repository, website) live here, keyed by id.
export const KNOWN_OFFICIAL_PLUGINS = {
  "lumia.annotation": {
    name: "Lumia Annotation",
    description:
      "Official annotation plugin: place text, rectangle, and numbered-step markers on an image and export a PNG, JPEG, or WebP copy.",
    tags: ["annotation", "markup", "official"],
    author: { name: "Lumia", url: "https://github.com/iFence" },
    repository: "https://github.com/iFence/lumia",
    website: "https://github.com/iFence/lumia",
  },
  "lumia.raw": {
    name: "Lumia RAW Preview",
    description:
      "Official camera RAW preview powered by LibRaw. Decodes DNG, CR2/CR3, NEF, ARW, RAF, ORF, RW2, and more to an orientation-corrected PNG preview.",
    tags: ["raw", "camera", "libraw", "official", "decoder"],
    author: { name: "Lumia", url: "https://github.com/iFence" },
    repository: "https://github.com/iFence/lumia",
    website: "https://github.com/iFence/lumia",
  },
};

function parseArguments(argv) {
  const values = new Map();
  for (let index = 0; index < argv.length; index += 2) {
    const key = argv[index];
    const value = argv[index + 1];
    if (!key?.startsWith("--") || value === undefined) {
      throw new Error(`invalid argument near ${key ?? "<end>"}`);
    }
    values.set(key.slice(2), value);
  }
  return values;
}

function required(values, name) {
  const value = values.get(name);
  if (!value) {
    throw new Error(`missing --${name}`);
  }
  return value;
}

function normalizeOs(value) {
  const normalized = value.toLowerCase();
  if (normalized === "win32" || normalized === "windows") return "windows";
  if (normalized === "darwin" || normalized === "macos") return "macos";
  if (normalized === "linux") return "linux";
  throw new Error(`unsupported target OS ${value}`);
}

export function normalizeArch(value) {
  const normalized = value.toLowerCase();
  if (["x64", "amd64", "x86_64"].includes(normalized)) return "x86_64";
  if (["arm64", "aarch64"].includes(normalized)) return "aarch64";
  throw new Error(`unsupported target architecture ${value}`);
}

async function walkFiles(root, relative = "") {
  const directory = path.join(root, relative);
  const entries = await readdir(directory, { withFileTypes: true });
  entries.sort((left, right) => left.name.localeCompare(right.name, "en"));
  const files = [];
  for (const entry of entries) {
    const child = path.join(relative, entry.name);
    const fullPath = path.join(root, child);
    const metadata = await lstat(fullPath);
    if (metadata.isSymbolicLink()) {
      throw new Error(`symbolic links are not allowed: ${child}`);
    }
    if (metadata.isDirectory()) {
      files.push(...(await walkFiles(root, child)));
    } else if (metadata.isFile()) {
      files.push(child);
    } else {
      throw new Error(`unsupported package entry: ${child}`);
    }
  }
  return files;
}

async function packageFile(root, relative) {
  const bytes = await readFile(path.join(root, relative));
  return {
    path: relative.split(path.sep).join("/"),
    size: bytes.length,
    sha256: createHash("sha256").update(bytes).digest("hex"),
  };
}

function privateKeyFrom(value) {
  if (value.includes("-----BEGIN")) {
    return createPrivateKey(value.replaceAll("\\n", "\n"));
  }
  return createPrivateKey({
    key: Buffer.from(value, "base64"),
    format: "der",
    type: "pkcs8",
  });
}

async function main() {
  const args = parseArguments(process.argv.slice(2));
  const stagingRoot = path.resolve(required(args, "root"));
  const targetOs = normalizeOs(required(args, "target-os"));
  const targetArch = normalizeArch(required(args, "target-arch"));
  const installDirectory = required(args, "install-directory");
  const minimumLumiaVersion = required(args, "minimum-lumia-version");
  const pluginApiVersion = Number(required(args, "plugin-api-version"));
  if (!Number.isSafeInteger(pluginApiVersion) || pluginApiVersion < 1) {
    throw new Error("plugin API version must be a positive integer");
  }

  const pluginRoot = path.join(stagingRoot, installDirectory);
  const runtimeManifestBytes = await readFile(
    path.join(pluginRoot, "lumia.plugin.json"),
  );
  const runtimeManifest = JSON.parse(runtimeManifestBytes);
  if (!OFFICIAL_PLUGIN_IDS.has(runtimeManifest.id)) {
    throw new Error(`unexpected plugin id ${runtimeManifest.id}`);
  }
  const expectedPluginId = args.get("plugin-id");
  if (expectedPluginId && runtimeManifest.id !== expectedPluginId) {
    throw new Error(`plugin id ${runtimeManifest.id} does not match ${expectedPluginId}`);
  }
  let keyValue = process.env.LUMIA_PLUGIN_SIGNING_KEY_PEM;
  if (args.has("private-key-file")) {
    keyValue = await readFile(path.resolve(args.get("private-key-file")), "utf8");
  }
  if (!keyValue) {
    throw new Error("LUMIA_PLUGIN_SIGNING_KEY_PEM is not configured");
  }
  const privateKey = privateKeyFrom(keyValue);
  const publicKey = createPublicKey(privateKey);
  const publicDer = publicKey.export({ format: "der", type: "spki" });
  const rawPublicKey = publicDer.subarray(publicDer.length - 32);
  const expectedPublicKey = Buffer.from(
    args.get("expected-public-key") ?? OFFICIAL_PUBLIC_KEY_HEX,
    "hex",
  );
  if (!rawPublicKey.equals(expectedPublicKey)) {
    throw new Error("signing key does not match Lumia's official plugin public key");
  }
  const runtimeSignature = sign(null, runtimeManifestBytes, privateKey);
  await writeFile(
    path.join(pluginRoot, "lumia.plugin.sig"),
    `${runtimeSignature.toString("base64")}\n`,
    "utf8",
  );

  const relativeFiles = await walkFiles(stagingRoot, installDirectory);
  const files = [];
  for (const relative of relativeFiles) {
    files.push(await packageFile(stagingRoot, relative));
  }

  const packageManifest = {
    schema_version: 1,
    plugin_id: runtimeManifest.id,
    version: runtimeManifest.version,
    plugin_api_version: pluginApiVersion,
    minimum_lumia_version: minimumLumiaVersion,
    target_os: targetOs,
    target_arch: targetArch,
    install_directory: installDirectory,
    files,
  };
  const manifestBytes = Buffer.from(
    `${JSON.stringify(packageManifest, null, 2)}\n`,
    "utf8",
  );

  const signature = sign(null, manifestBytes, privateKey);
  if (!verify(null, manifestBytes, publicKey, signature)) {
    throw new Error("package signature self-verification failed");
  }
  await writeFile(path.join(stagingRoot, "lumia.package.json"), manifestBytes);
  await writeFile(
    path.join(stagingRoot, "lumia.package.sig"),
    `${signature.toString("base64")}\n`,
    "utf8",
  );
  process.stdout.write(
    `Signed ${runtimeManifest.id} ${runtimeManifest.version} for ${targetOs}/${targetArch}\n`,
  );
}

// Run only when executed directly so `update-community-index.mjs` can import
// `normalizeArch` and `KNOWN_OFFICIAL_PLUGINS` without triggering a sign.
if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  main().catch((error) => {
    process.stderr.write(`${error.message}\n`);
    process.exitCode = 1;
  });
}
