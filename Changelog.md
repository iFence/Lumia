# Changelog

This file tracks the user-facing release notes for each version.

Conventions:
- Each version uses `## vX.Y.Z` as the heading, e.g. `## v0.1.3`
- The content below the heading is what appears in the in-app update dialog
- End each version section with `---` or by starting the next version heading

## v0.2.3

### ✨ New Features
- **SVG thumbnails in Explorer (Windows)**: after Lumia has run once, Windows Explorer shows real thumbnails for `.svg` and `.svgz` files in icon views instead of a generic file icon. The built-in thumbnail provider registers automatically on first launch and ships with the installer and portable packages; use `--register-thumbnail-handler` / `--unregister-thumbnail-handler` to manage it manually.

### 🐛 Bug Fixes
- **No stale raster when switching to SVG**: switching from a raster image (such as PNG or JPEG) to an SVG now retires the previous decoded image right away, releasing its pixel buffer promptly and letting the viewer fall through cleanly to SVG's path-based rendering instead of lingering on leftover pixels.

---

## v0.2.2

### ✨ New Features
- **Community plugin browser**: discover and install plugins from Settings > Plugins without downloading packages manually. The official RAW and Annotation plugins now appear in the browser and can be installed by search.
- **New annotation tools**: the Annotation plugin now offers text, rectangle, and numbered-step tools in place of the previous icon stamps, so you can mark up images the way you need.
- **Scale-aware annotation defaults**: annotation tool sizes automatically adapt to the image, so markers stay proportioned no matter the resolution.

### 🚀 Improvements
- **Safer edit workflow**: Lumia now asks for confirmation before discarding unapplied edits, so accidental switching won't lose your work.
- **More reliable settings overlay**: clicking inside the settings overlay no longer accidentally dismisses it.

---

## v0.2.1

### ✨ New Features
- **APNG and animated WebP playback**: animated PNG and WebP images now play alongside GIF using the core viewer's streaming animation path.
- **Bundled JPEG XL preview**: SDR still JPEG XL (`.jxl`) images can now be opened through an official process-isolated plugin.
- **Bundled JPEG 2000 preview**: JP2 and Part 1 codestream files (`.jp2`, `.j2k`, `.j2c`, `.jpc`) can now be previewed through an official process-isolated plugin.

### 🚀 Improvements
- **Safer animation decoding**: animation playback now observes frame memory limits, declared loop counts, cancellation, and bounded frame delays without collecting every frame in memory.
- **Complete format integration**: the new formats are available in file associations and are included with the official plugins in Windows, macOS, and Linux release packages.
- **Honest color handling**: HDR JPEG XL is rejected until Lumia has an HDR-capable display pipeline, preventing an SDR preview from being presented as accurate HDR output.

---

## v0.2.0

### ✨ New Features
- **RAW preview plugin**: install the optional RAW plugin to browse camera RAW photos (e.g. CR2, NEF, ARW).
- **View photo location on a map**: a new status-bar entry opens the photo's GPS shooting location in your system map.
- **Info panel quick actions**: the image-info overlay now has one-click copy and close buttons for easier EXIF viewing and sharing.
- **File association redesign**: the image format association settings have been overhauled for a more intuitive way to manage file types.
- **Context menu file operations**: right-click the viewer to copy or view EXIF info, copy the file path or open its location, or delete the current image to trash. Fullscreen toggle is also available from the context menu.

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
