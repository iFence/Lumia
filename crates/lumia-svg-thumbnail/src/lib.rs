//! Lumia's Windows Shell thumbnail provider for SVG.
//!
//! This crate builds `lumia-svg-thumbnail.dll`, an in-process COM server that
//! Explorer loads to render `.svg` / `.svgz` thumbnails. The shell hands the
//! provider the file contents through `IInitializeWithStream` and requests a
//! thumbnail through `IThumbnailProvider`. The rasterization itself is
//! cross-platform (`render.rs`) and unit-tested on every platform; the COM/GDI
//! plumbing is Windows-only.

mod render;

#[cfg(windows)]
mod com;
#[cfg(windows)]
mod dib;

#[cfg(windows)]
pub use com::{DllCanUnloadNow, DllGetClassObject};
