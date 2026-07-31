# Changelog

This file tracks the user-facing release notes for each version.

Conventions:
- Each version uses `## vX.Y.Z` as the heading, e.g. `## v0.1.3`
- The content below the heading is what appears in the in-app update dialog
- End each version section with `---` or by starting the next version heading

## v0.2.0

### ✨ New Features
- **RAW preview plugin**: install the optional RAW plugin to browse camera RAW photos (e.g. CR2, NEF, ARW).
- **View photo location on a map**: a new status-bar entry opens the photo's GPS shooting location in your system map.
- **Info panel quick actions**: the image-info overlay now has one-click copy and close buttons for easier EXIF viewing and sharing.
- **File association redesign**: the image format association settings have been overhauled for a more intuitive way to manage file types.

### 🚀 Improvements
- **More useful window title**: the title bar now shows the current image's position in its folder (e.g. "3/20"), so you can track browsing progress without opening the sidebar.
- **Cleaner large-image viewing**: fixed thin seam artifacts that could appear when zooming very large images.
- **More natural menu interactions**: edit and zoom menus now pop up right next to their trigger buttons; the status-bar edit menu toggles on click.
- **Smarter empty-state hints**: the drag-to-open hint now shows the shortcut for your operating system (⌘ on macOS, Ctrl on Windows).

---

## v0.1.5

### ✨ New Features
- **Signed plugin package installation**: install official signed `.lumiaplugin` packages from a file, with signature, platform, architecture, and integrity verified before install.
- **Update check on startup**: Lumia checks GitHub Releases for a newer version on launch and shows a hint in Settings > About.

### 🚀 Improvements
- **Improved README docs**: added version, download, license, and platform GitHub badges plus a Chinese README.
- Update checks can skip a specific version so you are not bothered repeatedly.

---

## v0.1.3

### ✨ New Features
- **Fast JPEG preview cache**: preloads adjacent files for smoother browsing.
- **Richer EXIF metadata**: parse and display more EXIF tags.

### 🚀 Improvements
- Fixed a CI release workflow permission issue.

---

## v0.1.2

### ✨ New Features
- First public preview release: basic image browsing, zoom, pan, and rotate.
- Folder navigation, EXIF info, file associations, and customizable shortcuts.

### 🚀 Improvements
- Startup speed and memory usage tuned for everyday browsing.

---
