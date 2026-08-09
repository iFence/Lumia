#!/usr/bin/env node

// Fixture test for `scripts/update-community-index.mjs`. Exercises the pure
// index-merge logic (`parseAssetName`, `updateIndex`) without network access.

import { readFile } from "node:fs/promises";

import { parseAssetName, updateIndex } from "./update-community-index.mjs";

async function readManifest(id) {
  const directory = id === "lumia.annotation" ? "lumia-plugin-annotation" : "lumia-plugin-raw";
  return JSON.parse(await readFile(`plugins/${directory}/lumia.plugin.json`, "utf8"));
}

function assert(condition, message) {
  if (!condition) throw new Error(message);
}

const manifests = {
  "lumia.annotation": await readManifest("lumia.annotation"),
  "lumia.raw": await readManifest("lumia.raw"),
};

// --- parseAssetName ----------------------------------------------------------

assert(parseAssetName("Lumia-RAW-windows-x64.lumiaplugin") === null, "legacy unversioned name must be skipped");
assert(parseAssetName("Lumia-macos-x64.dmg") === null, "non-plugin asset must be skipped");

const parsed = parseAssetName("Lumia-Annotation-0.2.0-macos-aarch64.lumiaplugin");
assert(parsed?.id === "lumia.annotation", "annotation id is parsed");
assert(parsed?.version === "0.2.0", "annotation version is parsed");
assert(parsed?.os === "macos", "macos os is parsed");
assert(parsed?.arch === "aarch64", "aarch64 arch is normalized");

const parsedX64 = parseAssetName("Lumia-RAW-0.1.0-windows-x64.lumiaplugin");
assert(parsedX64?.arch === "x86_64", "x64 arch is normalized to x86_64");
assert(parsedX64?.version === "0.1.0", "raw version is parsed");

// --- updateIndex: fresh merge ------------------------------------------------

const first = updateIndex(
  { schema_version: 1, index_version: "v0.2.1", plugins: [] },
  {
    tag: "v0.2.1",
    appVersion: "0.2.1",
    assets: [
      { id: "lumia.annotation", version: "0.2.0", os: "windows", arch: "x86_64", name: "Lumia-Annotation-0.2.0-windows-x86_64.lumiaplugin", size: 10, sha256: "a".repeat(64) },
      { id: "lumia.raw", version: "0.1.0", os: "windows", arch: "x86_64", name: "Lumia-RAW-0.1.0-windows-x86_64.lumiaplugin", size: 20, sha256: "b".repeat(64) },
    ],
    manifests,
  },
);
assert(first.plugins.length === 2, "two official plugins merged");
const annotation = first.plugins.find((plugin) => plugin.id === "lumia.annotation");
assert(annotation.name === "Lumia Annotation", "annotation name from known table");
assert(
  annotation.versions[0].artifacts[0].url.endsWith("/download/v0.2.1/Lumia-Annotation-0.2.0-windows-x86_64.lumiaplugin"),
  "artifact uses a fixed per-version URL",
);
assert(
  annotation.versions[0].install_directory === "lumia-plugin-annotation",
  "install_directory matches the manifest",
);
assert(annotation.versions[0].plugin_api_version === 3, "plugin API version is 3");
assert(
  annotation.permissions.length === 0 && annotation.permissions instanceof Array,
  "permissions read from manifest",
);

// --- updateIndex: preserves third-party + prior versions ---------------------

const sample = {
  id: "com.example.sample",
  name: "Sample Plugin",
  description: "third party",
  tags: ["sample"],
  permissions: [],
  versions: [],
};
const withPrior = updateIndex(
  {
    schema_version: 1,
    index_version: "v0.2.0",
    plugins: [
      sample,
      {
        id: "lumia.raw",
        name: "Lumia RAW Preview",
        description: "old",
        tags: [],
        permissions: [],
        versions: [
          {
            version: "0.0.9",
            minimum_lumia_version: "0.1.0",
            plugin_api_version: 3,
            install_directory: "lumia-plugin-raw",
            artifacts: [
              { target_os: "windows", target_arch: "x86_64", url: "https://old/0.0.9", sha256: "c".repeat(64), size: 1 },
            ],
          },
        ],
      },
    ],
  },
  {
    tag: "v0.2.1",
    appVersion: "0.2.1",
    assets: [
      { id: "lumia.raw", version: "0.1.0", os: "windows", arch: "x86_64", name: "Lumia-RAW-0.1.0-windows-x86_64.lumiaplugin", size: 20, sha256: "b".repeat(64) },
    ],
    manifests,
  },
);
const raw = withPrior.plugins.find((plugin) => plugin.id === "lumia.raw");
assert(
  raw.versions.map((version) => version.version).join(",") === "0.1.0,0.0.9",
  "newest version first, prior version retained",
);
assert(
  withPrior.plugins.some((plugin) => plugin.id === "com.example.sample"),
  "third-party plugin is preserved",
);
assert(
  withPrior.index_version === "v0.2.1",
  "index_version reflects the current release",
);

// --- updateIndex: replacing an artifact for the same (os, arch) ---------------

const refreshed = updateIndex(
  first,
  {
    tag: "v0.2.1",
    appVersion: "0.2.1",
    assets: [
      { id: "lumia.annotation", version: "0.2.0", os: "windows", arch: "x86_64", name: "Lumia-Annotation-0.2.0-windows-x86_64.lumiaplugin", size: 11, sha256: "d".repeat(64) },
    ],
    manifests,
  },
);
const annotationAgain = refreshed.plugins.find((plugin) => plugin.id === "lumia.annotation");
assert(
  annotationAgain.versions[0].artifacts.length === 1,
  "artifact replaced, not duplicated",
);
assert(
  annotationAgain.versions[0].artifacts[0].sha256 === "d".repeat(64),
  "artifact sha256 updated",
);

process.stdout.write("Community index fixture passed\n");
