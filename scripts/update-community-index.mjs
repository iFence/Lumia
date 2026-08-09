#!/usr/bin/env node

// Regenerates the community plugin index (`plugins.json`) in the
// `iFence/awesome-lumia-plugin` repo from the signed `.lumiaplugin` assets of
// a Lumia GitHub Release, then pushes it via the GitHub Contents API.
//
// Usage (env): GH_TOKEN, LUMA_PLUGIN_TAG (e.g. v0.2.1)
//   - GH_TOKEN: a fine-grained PAT with Contents read/write on the index repo.
//   - LUMA_PLUGIN_TAG: the git tag of the release whose assets are indexed.
//
// The script reads the plugin manifests from the local checkout for
// `permissions`, so it must run from a Lumia checkout at the release commit.
//
// It is best-effort by design: CI wraps failures so a release still succeeds.
// It never prints the token.

import { createHash } from "node:crypto";
import { readFile } from "node:fs/promises";
import { pathToFileURL } from "node:url";

import {
  KNOWN_OFFICIAL_PLUGINS,
  normalizeArch,
} from "./sign-plugin-package.mjs";

const LUMA_OWNER = "iFence";
const LUMA_REPO = "lumia";
const INDEX_OWNER = "iFence";
const INDEX_REPO = "awesome-lumia-plugin";
const INDEX_PATH = "plugins.json";
const PLUGIN_API_VERSION = 3;

const INSTALL_DIRECTORIES = {
  "lumia.annotation": "lumia-plugin-annotation",
  "lumia.raw": "lumia-plugin-raw",
};

const SUPPORTED_OS = new Set(["windows", "macos", "linux"]);
const SEMVER_RE = /^\d+\.\d+\.\d+/;

/// Parses a versioned release asset name into `{ id, version, os, arch }`.
/// Returns `null` for names that are not official plugin packages (MSIs,
/// zips, dmgs) and for the legacy unversioned names (`Lumia-RAW-windows-x64`),
/// which are published for backward compatibility but are not indexed.
export function parseAssetName(name) {
  const match = /^Lumia-(RAW|Annotation)-(.+)\.lumiaplugin$/.exec(name);
  if (!match) return null;
  const parts = match[2].split("-");
  if (parts.length < 3) return null;
  if (!SEMVER_RE.test(parts[0])) return null;
  const version = parts.shift();
  if (parts.length !== 2) return null;
  const [os, arch] = parts;
  if (!SUPPORTED_OS.has(os)) return null;
  let normalizedArch;
  try {
    normalizedArch = normalizeArch(arch);
  } catch {
    return null;
  }
  return {
    id: match[1] === "RAW" ? "lumia.raw" : "lumia.annotation",
    version,
    os,
    arch: normalizedArch,
  };
}

function compareArtifacts(a, b) {
  return (
    a.target_os.localeCompare(b.target_os) ||
    a.target_arch.localeCompare(b.target_arch)
  );
}

function compareSemver(a, b) {
  const pa = a.split(".").map(Number);
  const pb = b.split(".").map(Number);
  for (let index = 0; index < 3; index += 1) {
    const left = pa[index] ?? 0;
    const right = pb[index] ?? 0;
    if (left !== right) return left - right;
  }
  return 0;
}

/// Merges the current release's artifacts into the existing index. Prior
/// versions of the official plugins are preserved so old versions stay
/// installable and the UI can show an Update action. Third-party plugins are
/// never touched.
///
/// `assets` is the parsed, sha256-computed list from the current release;
/// `manifests` maps plugin id to the local `lumia.plugin.json` for permissions.
export function updateIndex(existing, { tag, appVersion, assets, manifests }) {
  const priorById = new Map(existing.plugins.map((plugin) => [plugin.id, plugin]));
  const kept = existing.plugins.filter(
    (plugin) => !KNOWN_OFFICIAL_PLUGINS[plugin.id],
  );
  const official = [];

  for (const id of Object.keys(KNOWN_OFFICIAL_PLUGINS).sort()) {
    const meta = KNOWN_OFFICIAL_PLUGINS[id];
    const prior = priorById.get(id);
    const current = assets.filter((asset) => asset.id === id && asset.version);
    if (current.length === 0 && !prior) continue;

    const versionsByKey = new Map();
    for (const version of prior?.versions ?? []) {
      versionsByKey.set(version.version, {
        version: version.version,
        minimum_lumia_version: version.minimum_lumia_version,
        plugin_api_version: version.plugin_api_version,
        install_directory: INSTALL_DIRECTORIES[id],
        artifacts: [...version.artifacts],
      });
    }
    const currentByVersion = new Map();
    for (const asset of current) {
      const artifacts = currentByVersion.get(asset.version) ?? [];
      artifacts.push({
        target_os: asset.os,
        target_arch: asset.arch,
        url: `https://github.com/${LUMA_OWNER}/${LUMA_REPO}/releases/download/${tag}/${asset.name}`,
        sha256: asset.sha256,
        size: asset.size,
      });
      currentByVersion.set(asset.version, artifacts);
    }
    for (const [version, newArtifacts] of currentByVersion) {
      const entry =
        versionsByKey.get(version) ??
        {
          version,
          minimum_lumia_version: appVersion,
          plugin_api_version: PLUGIN_API_VERSION,
          install_directory: INSTALL_DIRECTORIES[id],
          artifacts: [],
        };
      const replaced = entry.artifacts.filter(
        (artifact) =>
          !newArtifacts.some(
            (next) =>
              next.target_os === artifact.target_os &&
              next.target_arch === artifact.target_arch,
          ),
      );
      entry.artifacts = [...replaced, ...newArtifacts].sort(compareArtifacts);
      entry.minimum_lumia_version = appVersion;
      entry.plugin_api_version = PLUGIN_API_VERSION;
      versionsByKey.set(version, entry);
    }
    const versions = [...versionsByKey.values()].sort((left, right) =>
      compareSemver(right.version, left.version),
    );

    official.push({
      id,
      name: meta.name,
      description: meta.description,
      author: meta.author,
      tags: [...meta.tags],
      permissions: manifests[id]?.permissions ?? prior?.permissions ?? [],
      repository: meta.repository,
      website: meta.website,
      versions,
    });
  }

  const plugins = [...kept, ...official].sort((left, right) =>
    left.id.localeCompare(right.id),
  );
  return { schema_version: existing.schema_version, index_version: tag, plugins };
}

async function downloadAssetSha256(url) {
  // Public repo: the browser_download_url redirects to a signed storage URL,
  // so no Authorization header is attached here.
  const response = await fetch(url, { headers: { "User-Agent": "lumia-release" } });
  if (!response.ok) {
    throw new Error(`cannot download ${url}: HTTP ${response.status}`);
  }
  const hasher = createHash("sha256");
  const reader = response.body.getReader();
  while (true) {
    const { done, value } = await reader.read();
    if (done) break;
    hasher.update(value);
  }
  return hasher.digest("hex");
}

async function main() {
  const token = process.env.GH_TOKEN;
  if (!token) throw new Error("GH_TOKEN is not configured");
  const tag = process.env.LUMA_PLUGIN_TAG;
  if (!tag) throw new Error("LUMA_PLUGIN_TAG is not configured");
  const appVersion = tag.replace(/^v/, "");
  if (!SEMVER_RE.test(appVersion)) throw new Error(`invalid LUMA_PLUGIN_TAG ${tag}`);

  const headers = {
    Authorization: `Bearer ${token}`,
    Accept: "application/vnd.github+json",
    "User-Agent": "lumia-release",
    "X-GitHub-Api-Version": "2022-11-28",
  };

  const releaseResponse = await fetch(
    `https://api.github.com/repos/${LUMA_OWNER}/${LUMA_REPO}/releases/tags/${tag}`,
    { headers },
  );
  if (releaseResponse.status !== 200) {
    throw new Error(`cannot read release ${tag}: HTTP ${releaseResponse.status}`);
  }
  const release = await releaseResponse.json();

  const assets = [];
  for (const asset of release.assets ?? []) {
    const parsed = parseAssetName(asset.name);
    if (!parsed || !parsed.version) continue;
    assets.push({ ...parsed, name: asset.name, size: asset.size, url: asset.browser_download_url });
  }
  if (assets.length === 0) {
    throw new Error(`no versioned Lumia plugin assets found on release ${tag}`);
  }
  for (const asset of assets) {
    asset.sha256 = await downloadAssetSha256(asset.url);
  }

  const manifests = {};
  for (const id of Object.keys(KNOWN_OFFICIAL_PLUGINS)) {
    const directory = INSTALL_DIRECTORIES[id];
    try {
      manifests[id] = JSON.parse(
        await readFile(`plugins/${directory}/lumia.plugin.json`, "utf8"),
      );
    } catch (error) {
      throw new Error(`cannot read plugins/${directory}/lumia.plugin.json: ${error.message}`);
    }
  }

  const indexResponse = await fetch(
    `https://api.github.com/repos/${INDEX_OWNER}/${INDEX_REPO}/contents/${INDEX_PATH}`,
    { headers },
  );
  if (indexResponse.status !== 200) {
    throw new Error(
      `cannot read ${INDEX_OWNER}/${INDEX_REPO} ${INDEX_PATH}: HTTP ${indexResponse.status}`,
    );
  }
  const indexBody = await indexResponse.json();
  const existing = JSON.parse(Buffer.from(indexBody.content, "base64").toString("utf8"));
  if (existing.schema_version !== 1) {
    throw new Error(
      `unsupported index schema_version ${existing.schema_version}; refusing to modify`,
    );
  }

  const next = updateIndex(existing, { tag, appVersion, assets, manifests });
  const output = `${JSON.stringify(next, null, 2)}\n`;

  const putBody = {
    message: `Update community index for Lumia ${tag}`,
    content: Buffer.from(output, "utf8").toString("base64"),
    sha: indexBody.sha,
  };
  const putResponse = await fetch(
    `https://api.github.com/repos/${INDEX_OWNER}/${INDEX_REPO}/contents/${INDEX_PATH}`,
    {
      method: "PUT",
      headers: { ...headers, "Content-Type": "application/json" },
      body: JSON.stringify(putBody),
    },
  );
  if (putResponse.status !== 200 && putResponse.status !== 201) {
    const detail = await putResponse.text();
    throw new Error(
      `cannot update index: HTTP ${putResponse.status} ${detail.slice(0, 300)}`,
    );
  }
  process.stdout.write(
    `Updated community index for ${tag}: ${next.plugins.length} plugins\n`,
  );
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  main().catch((error) => {
    process.stderr.write(`${error.message}\n`);
    process.exitCode = 1;
  });
}
