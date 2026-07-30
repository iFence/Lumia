#!/usr/bin/env node

import { createHash } from "node:crypto";
import { createReadStream, createWriteStream } from "node:fs";
import { mkdir, readFile, readdir, rename, rm, stat, writeFile } from "node:fs/promises";
import { resolve, join } from "node:path";
import { Readable } from "node:stream";
import { pipeline } from "node:stream/promises";

const license = {
  name: "Creative Commons CC0 1.0 Universal",
  url: "https://creativecommons.org/publicdomain/zero/1.0/",
};

const samples = [
  {
    extension: "dng",
    camera: "Sigma fp",
    url: "https://raw.pixls.us/getfile.php/7280/nice/Sigma - fp - 8bit (16:9).DNG",
    sha256: "80ae0b8fdce3f58286fc194513c6ec0080ab07caca753408b00ca72c90e5ad0c",
  },
  {
    extension: "cr3",
    camera: "Canon EOS R6",
    url: "https://raw.pixls.us/getfile.php/4659/nice/Canon - EOS R6 - 3:2.CR3",
    sha256: "74abb0a113d075ad9887a058082f40dd2a938c4813a08474d82356f11a027778",
  },
  {
    extension: "nef",
    camera: "Nikon D2H",
    url: "https://raw.pixls.us/getfile.php/5227/nice/Nikon - D2H - 12bit 12bit compressed (Lossy (type 1)) (3:2).NEF",
    sha256: "155edb938f884ea7372ce98d4ff5f965c3e413b43b95bc9923da6e92082cf914",
  },
  {
    extension: "arw",
    camera: "Sony ILCE-7S",
    url: "https://raw.pixls.us/getfile.php/1582/nice/Sony - ILCE-7S - 14bit 14bit compressed (3:2).ARW",
    sha256: "a35ebb2fbec929daa5beb20d1ce5c15a8aac7b1a7a231455387f3df8a7442e07",
  },
  {
    extension: "raf",
    camera: "Fujifilm FinePix S5000",
    url: "https://raw.pixls.us/getfile.php/2726/nice/Fujifilm - FinePix S5000 - 4:3.RAF",
    sha256: "dabd5e74521a6980156be9fd4b88d0c37b0fe4d0e0e6f5c12db8cffff1b76297",
  },
];

const outputIndex = process.argv.indexOf("--output");
if (outputIndex >= 0 && !process.argv[outputIndex + 1]) {
  throw new Error("--output requires a directory");
}
const outputDirectory = resolve(
  outputIndex >= 0 ? process.argv[outputIndex + 1] : "target/raw-test-samples",
);
await mkdir(outputDirectory, { recursive: true });

async function fileHash(path) {
  const hash = createHash("sha256");
  await pipeline(createReadStream(path), hash);
  return hash.digest("hex");
}

async function isCurrent(path, expectedHash) {
  try {
    await stat(path);
    return (await fileHash(path)) === expectedHash;
  } catch (error) {
    if (error?.code === "ENOENT") return false;
    throw error;
  }
}

for (const sample of samples) {
  const destination = join(outputDirectory, `sample.${sample.extension}`);
  if (await isCurrent(destination, sample.sha256)) {
    process.stdout.write(`Verified cached ${sample.extension.toUpperCase()} sample\n`);
    continue;
  }

  await rm(destination, { force: true });
  const partial = `${destination}.${process.pid}.part`;
  for (const entry of await readdir(outputDirectory)) {
    if (entry.startsWith(`sample.${sample.extension}.`) && entry.endsWith(".part")) {
      await rm(join(outputDirectory, entry), { force: true });
    }
  }

  await rm(partial, { force: true });
  const response = await fetch(sample.url, {
    headers: { "user-agent": "Lumia RAW plugin integration tests" },
    redirect: "follow",
  });
  if (!response.ok || !response.body) {
    throw new Error(`Could not download ${sample.extension}: HTTP ${response.status}`);
  }
  try {
    await pipeline(Readable.fromWeb(response.body), createWriteStream(partial, { flags: "wx" }));
    const actualHash = await fileHash(partial);
    if (actualHash !== sample.sha256) {
      throw new Error(
        `${sample.extension} checksum mismatch: expected ${sample.sha256}, found ${actualHash}`,
      );
    }
    await rename(partial, destination);
  } catch (error) {
    await rm(partial, { force: true });
    throw error;
  }
  process.stdout.write(`Downloaded and verified ${sample.extension.toUpperCase()} sample\n`);
}

const dng = await readFile(join(outputDirectory, "sample.dng"));
await writeFile(join(outputDirectory, "corrupt.dng"), dng.subarray(0, Math.min(4096, dng.length)));
await writeFile(
  join(outputDirectory, "SOURCES.json"),
  `${JSON.stringify({ license, samples }, null, 2)}\n`,
);
