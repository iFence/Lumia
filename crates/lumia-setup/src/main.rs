#![cfg_attr(windows, windows_subsystem = "windows")]

#[cfg(windows)]
mod dialog;
#[cfg(windows)]
mod embedded;
#[cfg(windows)]
mod installer;
#[cfg(windows)]
mod legacy;
#[cfg(windows)]
mod setup;

#[cfg(windows)]
fn main() {
    setup::run();
}

#[cfg(not(windows))]
fn main() {
    eprintln!("Lumia Setup is available only on Windows.");
}
