# Lumia Audit Addendum — Supply Chain, Secrets & CI

**Date:** 2026-08-29
**Scope:** gaps not covered by `AUDIT-2026-08-29.md` — dependency vulnerabilities,
supply-chain pinning, secret hygiene, CI/CD pipeline, and `unsafe` usage.
**Same baseline:** commit `2f1afd7` + working tree (31 modified files).

The original audit covered architecture, code quality, and the plugin sandbox. It
did **not** run a dependency vulnerability scan, review CI, or examine the
signing/release pipeline. This addendum closes those gaps.

## Executive summary

No secrets are leaked, and the signing key material is handled correctly — that
part is genuinely well done. The new exposure is **supply chain and CI
enforcement**: the project carries 7 known-vulnerable dependencies, one of which
sits on the live network path, and **neither `cargo clippy` nor any dependency
scanner runs in CI**. Two vulnerabilities are real; five are build-time or
unreachable once traced.

Severity counts: **0 critical · 1 high · 4 medium · 6 low**.

---

## HIGH

### S1 — Vulnerable dependencies present, and no scanner runs in CI

`cargo audit` (907 crate deps) reports **7 vulnerabilities and 11 warnings**.
This had never been run before, and nothing in CI would catch a regression.

| Crate | Version | Advisory | Severity | Fix |
|---|---|---|---|---|
| `h2` | 0.4.14 | RUSTSEC-2026-0258 unbounded empty DATA frames | — | >=0.4.16 |
| `quinn-proto` | 0.11.14 | RUSTSEC-2026-0185 remote memory exhaustion | 7.5 high | >=0.11.15 |
| `crossbeam-epoch` | 0.9.18 | RUSTSEC-2026-0204 invalid pointer deref | — | >=0.9.20 |
| `quick-xml` | 0.30.0, 0.39.4 | RUSTSEC-2026-0194 / -0195 (×2 each) | 7.5 high | >=0.41.0 |

Reachability triage — this is where the raw count is misleading:

- **`h2` — genuinely reachable.** `lumia-app → reqwest_client → zed-reqwest →
  hyper-rustls → hyper → h2`. This is the HTTP/2 stack behind every network call
  in the app (update check, community index, `download_image`). This compounds
  with finding **H3** in the main audit (no read timeouts): a malicious or
  compromised server can hold connections open indefinitely *and* exploit the
  frame-handling flaw. **This is the one to fix.**
- **`quinn-proto` and `quick-xml 0.30.0` — not in the reachable build graph.**
  Present in `Cargo.lock` but absent from `cargo tree`; stale lockfile entries
  for this platform/target.
- **`quick-xml 0.39.4` — reachable but not exploitable.** Path is
  `zbus_xml → zbus-lockstep → zbus`. Three mitigations stack: it is a
  **build-time proc-macro** dependency, it is **Linux-only** (D-Bus), and the XML
  it parses is **developer-authored interface definitions**, not attacker input.
  The 7.5 severity does not apply to this call path.
- **`crossbeam-epoch` — build-time only.** Path is
  `crossbeam-deque → ignore → globwalk → rust-i18n-support → rust-i18n-macro`
  (proc-macro). Affects the build machine, not shipped binaries.

**Warnings (11), all transitive:** unmaintained — `instant`, `paste`,
`rustls-pemfile`, `rustybuzz`, `ttf-parser`; unsound — `anyhow`,
`event-listener`, `memmap2`; yanked — `spin` 0.9.8 and 0.10.0.

**Remediation**
1. Add `cargo audit` (or `cargo deny`) to CI as a required check. Without this,
   every other fix below silently regresses.
2. Bump `h2` to >=0.4.16 — may need `[patch]` or a `cargo update -p h2`, since it
   arrives transitively through the zed git dependency.
3. Bump `crossbeam-epoch` and `quinn-proto` opportunistically; they cost nothing.

---

## MEDIUM

### S2 — Six git dependencies, four of them unpinned

`Cargo.toml:29-34` declares six git dependencies:

```toml
gpui                  = { git = "https://github.com/zed-industries/zed" }          # unpinned
gpui_platform         = { git = "https://github.com/zed-industries/zed", ... }      # unpinned
http_client           = { git = "https://github.com/zed-industries/zed" }           # unpinned
reqwest_client        = { git = "https://github.com/zed-industries/zed" }           # unpinned
gpui-component        = { git = "https://github.com/longbridge/gpui-component" }    # unpinned
gpui-component-assets = { git = "https://github.com/longbridge/gpui-component" }    # unpinned
```

None specifies `rev`, `tag`, or `branch`. They float on the upstream default
branch; `Cargo.lock` currently pins `zed#a50a292f…` and
`gpui-component#bc174a7e…`. Any `cargo update`, lockfile regeneration, or fresh
clone without the lock can silently pull a different build of the entire UI
toolkit. For an app whose plugin sandbox is otherwise carefully hardened, the
primary UI dependency is the least-controlled input in the tree.

`AGENTS.md` requires an ADR for dependency policy changes; no ADR covers this.

**Remediation:** pin all six to an explicit `rev`. Note the contrast with
`font-kit`, `reqwest.git`, and `scap`, which *are* correctly pinned to `rev=` —
the discipline exists, it is just applied inconsistently.

### S3 — `cargo clippy` does not run in CI

The main audit states "clippy 0 errors" as a baseline, but **CI never invokes
clippy**. `.github/workflows/ci.yml` runs `cargo fmt --check`,
`cargo check --workspace --all-targets`, and `cargo test --workspace` only. The
clean lint state is therefore a local convention, not an enforced gate — nothing
prevents a lint regression from merging.

**Remediation:** add `cargo clippy --workspace --all-targets -- -D warnings` to CI.

### S4 — CI overrides the pinned toolchain

`rust-toolchain.toml` pins `channel = "1.95.0"` with `clippy` as a component, but
every workflow job uses `dtolnay/rust-toolchain@stable`, which installs the
latest stable and **overrides** the pin. CI and local development therefore
compile with different compilers, and a toolchain upgrade can break CI
independently of any code change.

**Remediation:** drop the explicit toolchain step and let `rust-toolchain.toml`
govern (all current actions respect it), or pin the action to `1.95.0`.

### S5 — Third-party actions are not SHA-pinned

`actions/checkout@v4`, `actions/setup-node@v4`, and
`softprops/action-gh-release@v2` are referenced by floating major tag. A
compromised or retagged action executes with repository and secret access — and
these workflows handle the plugin signing key (see "Verified clean").

**Remediation:** pin to full commit SHAs.

---

## LOW

- **S6 — `memmap2` 0.9.10 is a direct dependency with a soundness advisory.**
  Declared at `Cargo.toml:45`, used by `lumia-core` in
  `image/large/cache.rs:134,201` and `image/large/mapped.rs:48,187`.
  RUSTSEC-2026-0186 affects only the *range* variants —
  `Mmap::[unchecked_]advise_range` and `MmapMut::[unchecked_]advise_range`,
  `flush_range`, `flush_async_range`. **Verified not currently exploitable:**
  Lumia calls only `Mmap::map`, `MmapMut::map_mut`, and `map.flush()`
  (`cache.rs:162`), none of which are affected. Still worth bumping to 0.9.11 —
  it is a one-line change and the affected APIs are a natural thing for someone
  to reach for later.
- **S7 — `anyhow` 1.0.102** flagged unsound (`Error::downcast_mut`). 1.0.104 is
  available; the advisory records no patched version. Low practical risk.
- **S8 — Stale `Cargo.lock` entries.** `quinn-proto` and `quick-xml` 0.30.0 are
  locked but unreachable in this build graph.
- **S9 — Secret naming inconsistency.** `release.yml:313` uses
  `secrets.LUMA_INDEX_ACCESS_TOKEN` ("LUMA", not "LUMIA"). Probably a legacy
  name, but a typo here is exactly the kind of thing that silently breaks a
  release job.
- **S10 — `resvg` version coupling** (carried from M2/L3 in the main audit).
- **S11 — Untracked directories.** `.opencode/` is untracked and unignored.

---

## Verified clean

These were actively checked and are sound — worth recording so they are not
re-litigated later.

**No secrets committed.** A secret scan across the tree returned two hits, both
false positives, both confirmed by reading the source:
- `docs/官方插件签名与密钥管理指南.md:33` — `-----BEGIN PRIVATE KEY-----` is a
  PEM *format template* inside a documentation code fence; the body is literally
  `...`.
- The 64-hex string at `docs/…:19` is explicitly labelled the **public** key.

**Signing key is consistent across all three representations.** The public key
`6b88de1c86a73ae6…` in the docs, `scripts/sign-plugin-package.mjs:15`
(`OFFICIAL_PUBLIC_KEY_HEX`), and the compiled `crates/lumia-app/src/plugin_package.rs:29`
byte array (`0x6b, 0x88, 0xde, 0x1c, 0x86, 0xa7, 0x3a, 0xe6, 0x66, 0xd4, 0xa4, 0x4b, 0x54, 0xe3, 0x04, 0x69`)
all agree. No key drift — which is the failure mode that would most quietly break
plugin verification.

**Private key handling in CI is correct.** The key is passed as
`secrets.LUMIA_PLUGIN_SIGNING_KEY_PEM` via environment variables only
(`release.yml:95,115,166,182,267,283`), never written to disk.
`scripts/build-local-annotation-plugin.ps1` explicitly clears the env var after
use (line 67). The signing script is documented as never printing the key.

**`unsafe` usage is confined to FFI boundaries.** Every occurrence is a
Windows COM/Win32 call (`lumia-svg-thumbnail`, `lumia-app/src/shell/windows.rs`,
`single_instance/windows.rs`, `lumia-setup`), a macOS FFI call
(`shell/macos.rs`), an `mmap` call (`lumia-core/src/image/large/`), or a C ABI
bridge (`plugins/lumia-plugin-raw/src/bridge.rs`). No gratuitous `unsafe` in
pure-Rust logic.

**Native binary downloads are hash-pinned.** `scripts/build-raw-native.ps1:23-24`
pins SHA-256 for both the LibRaw and CMake archives.

---

## Recommended order of work

1. **S1** — add `cargo audit` to CI (without this, nothing else stays fixed).
2. **S1** — bump `h2` to >=0.4.16; it is reachable on the live network path.
3. **S3** — add `cargo clippy -D warnings` to CI.
4. **S2** — pin all six git dependencies to explicit revisions.
5. **S4** — reconcile `rust-toolchain.toml` with CI.
6. **S5** — SHA-pin third-party GitHub Actions.
7. **S6/S7/S8** — opportunistic dependency bumps.

Combined with the main audit, the two items that warrant real urgency are **H2**
(self-update executes unverified binaries) and **S1/S3** (CI enforces neither
dependency security nor lints, which is how both categories drift silently).
