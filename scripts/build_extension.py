#!/usr/bin/env python3
import os
import json
import shutil
import zipfile

PROJECT_ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
EXT_SRC = os.path.join(PROJECT_ROOT, 'extension')
DIST_DIR = os.path.join(PROJECT_ROOT, 'dist')
TEMP_DIR = os.path.join(PROJECT_ROOT, 'temp_ext_build')

def clean_directory(path):
    if os.path.exists(path):
        shutil.rmtree(path)

def zip_directory(src_dir, zip_path):
    with zipfile.ZipFile(zip_path, 'w', zipfile.ZIP_DEFLATED) as zipf:
        for root, _, files in os.walk(src_dir):
            for file in files:
                abs_path = os.path.join(root, file)
                rel_path = os.path.relpath(abs_path, src_dir)
                zipf.write(abs_path, rel_path)

def build():
    print('Building rs-summarizer browser extensions...')

    # Ensure dist folder exists
    if not os.path.exists(DIST_DIR):
        os.makedirs(DIST_DIR)

    # Define build paths
    chrome_temp = os.path.join(TEMP_DIR, 'chrome')
    firefox_temp = os.path.join(TEMP_DIR, 'firefox')

    clean_directory(TEMP_DIR)
    os.makedirs(chrome_temp, exist_ok=True)
    os.makedirs(firefox_temp, exist_ok=True)

    # 1. Package Chrome Extension
    print('Preparing Chrome extension files...')
    shutil.copytree(EXT_SRC, chrome_temp, dirs_exist_ok=True)
    
    # Read and strip Firefox-specific settings for Chrome manifest
    chrome_manifest_path = os.path.join(chrome_temp, 'manifest.json')
    with open(chrome_manifest_path, 'r', encoding='utf-8') as f:
        manifest = json.load(f)
    
    if 'browser_specific_settings' in manifest:
        del manifest['browser_specific_settings']
        
    with open(chrome_manifest_path, 'w', encoding='utf-8') as f:
        json.dump(manifest, f, indent=2)

    # Zip Chrome extension
    chrome_zip_path = os.path.join(DIST_DIR, 'rs-summarizer-chrome.zip')
    if os.path.exists(chrome_zip_path):
        os.remove(chrome_zip_path)
    
    print('Zipping Chrome extension...')
    zip_directory(chrome_temp, chrome_zip_path)
    print(f'Chrome extension packaged successfully: {chrome_zip_path}')

    # 2. Package Firefox Extension
    print('Preparing Firefox extension files...')
    shutil.copytree(EXT_SRC, firefox_temp, dirs_exist_ok=True)
    
    # Firefox uses the manifest.json with browser_specific_settings intact.

    # Zip Firefox extension
    firefox_zip_path = os.path.join(DIST_DIR, 'rs-summarizer-firefox.zip')
    if os.path.exists(firefox_zip_path):
        os.remove(firefox_zip_path)

    print('Zipping Firefox extension...')
    zip_directory(firefox_temp, firefox_zip_path)
    print(f'Firefox extension packaged successfully: {firefox_zip_path}')

    # Cleanup temp folder
    print('Cleaning up temporary build files...')
    clean_directory(TEMP_DIR)
    
    print('All extensions built successfully!')

if __name__ == '__main__':
    build()
