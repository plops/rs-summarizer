# Local Testing Guide for rs-summarizer Extension

This directory contains the unified browser extension codebase for `rs-summarizer` (supporting Manifest V3 in both Google Chrome and Mozilla Firefox).

## Default Settings
- The extension defaults to pointing to the production website at `https://rocketrecap.com`.
- If you are running the Rust server locally, expand **Advanced Options** in the extension popup and change the **Rust Server URL** to `http://localhost:5001` (or whichever port your server is listening on).

---

## 1. Testing in Google Chrome

You can load your unpacked development folder directly into Chrome without needing a developer account.

### Method 1: Manual Testing via chrome://extensions
1. Open Google Chrome and type `chrome://extensions/` in the address bar.
2. Toggle the **Developer mode** switch in the top-right corner to **On**.
3. Click the **Load unpacked** button that appears in the top-left corner.
4. Select the `extension` folder in your project directory (the folder containing `manifest.json`).
5. Open any public website (e.g., a news article or YouTube video), click the extension icon in your toolbar, choose your options, and click **Summarize**.

*Note: If you make changes to your codebase, you must click the **Refresh icon** (circular arrow) on the extension's card in `chrome://extensions/` to apply them.*

### Method 2: Automated Testing via `web-ext`
The official Mozilla `web-ext` tool can launch and test your extension automatically in Chrome with live-reloads:
1. Ensure Node.js is installed, and install the tool globally or in your project:
   ```bash
   npm install --save-dev web-ext
   ```
2. Build the extension package:
   ```bash
   python3 scripts/build_extension.py
   ```
3. Run the automated environment targeting Chrome/Chromium:
   ```bash
   npx web-ext run --target=chromium --source-dir ./extension
   ```

---

## 2. Testing in Mozilla Firefox

Firefox refers to extensions as "Add-ons." You can load it directly as a temporary add-on.

### Method 1: Manual Testing via `about:debugging`
1. Open Firefox and type `about:debugging` in the address bar.
2. Click on **This Firefox** in the left-hand sidebar.
3. Click the **Load Temporary Add-on...** button.
4. Navigate to the `extension` folder and select the `manifest.json` file.
5. Open any webpage or YouTube video, click the extension icon in the toolbar, and verify it functions correctly.

*Note: Extensions loaded via this method are temporary and will disappear when you close Firefox.*

### Method 2: Automated Testing via `web-ext` (Recommended)
`web-ext` launches an isolated, temporary Firefox profile with your extension pre-loaded and reloads automatically on code saves:
1. Install `web-ext`:
   ```bash
   npm install --save-dev web-ext
   ```
2. Run the extension in Firefox:
   ```bash
   npx web-ext run --source-dir ./extension
   ```
3. Keep the terminal open. Whenever you save changes to your Javascript or HTML files, the extension will instantly reload in the test browser.

---

## Key Debugging Tools

### Google Chrome
- **Popup/Options logs**: Right-click anywhere in the open extension popup and select **Inspect**.
- **Page script injection logs**: Press `F12` on the webpage you are testing. Console logs from page script injection appear here.

### Mozilla Firefox
- **Popup/Options logs**: On the `about:debugging` page, locate your extension and click the **Inspect** button. This opens a developer tools window showing console statements and network fetch logs.
- **Page script injection logs**: Press `F12` on the webpage you are testing. Injection logs print to the standard webpage console.
