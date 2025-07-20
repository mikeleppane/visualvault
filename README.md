# 🖼️ VisualVault
A modern, terminal-based media file organizer built with Rust

![CI](https://github.com/mikeleppane/visualvault/workflows/CI/badge.svg)
<img alt="Status" src="https://img.shields.io/badge/Status-Work in Progress-yellow">
<img alt="Rust" src="https://img.shields.io/badge/Rust-1.85-orange">
<img alt="License" src="https://img.shields.io/badge/License-MIT-blue">


## 🎥 Introduction Videos

[![Watch the video](https://img.youtube.com/vi/JdzuCGQH1vQ/maxresdefault.jpg)](https://youtu.be/JdzuCGQH1vQ)

<p align="center">
  <i>Click the image above to watch a quick introduction to VisualVault</i>
</p>


[![Watch the video](https://img.youtube.com/vi/uvDJqplAudA/maxresdefault.jpg)](https://youtu.be/uvDJqplAudA)

<p align="center">
  <i>Click the image above to see VisualVault's Duplicate Detector in action</i>
</p>

## 📸 Screenshot

<p align="center">
  <img src="images/screenshot.png" alt="VisualVault Screenshot" />
</p>

## ✨ Features
### 🎯 Core Functionality
Smart Organization: Automatically organize media files by date, type, or custom rules  
Duplicate Detection: Find and manage duplicate files across your collection  
Metadata Extraction: Extract EXIF data from images for intelligent organization  
Batch Processing: Handle thousands of files efficiently with async operations  
### 🖥️ Terminal UI
Modern TUI: Beautiful terminal interface built with Ratatui  
Real-time Progress: Live progress tracking for all operations  
Interactive Dashboard: View statistics and insights about your media collection  
Keyboard Navigation: Fully keyboard-driven interface for power users  
### ⚡ Performance
Async/Await: Built on Tokio for blazing-fast concurrent operations  
Configurable Workers: Adjust thread count for optimal performance  
Smart Caching: Efficient file metadata caching  
SSD Optimization: Special settings for solid-state drives  

## 🚀 Getting Started
Prerequisites
 * Rust 1.75 or higher
 * Linux, macOS, or Windows

Installation

```bash
# Clone the repository
git clone https://github.com/yourusername/visualvault.git
cd visualvault

# Build the project
cargo build --release

# Run the application
cargo run --release
```

Quick Start
 1. Launch VisualVault:
```bash
cargo run --release
```

 2. Configure source and destination folders:
    * Press s to open Settings
    * Set your source folder (where your media files are)
    * Set your destination folder (where organized files will go)
 3. Start organizing:
    * Press r to scan for files
    * Press o to organize them

## 🎮 Keyboard Shortcuts
Global
 * `?` or `F1` - Show help
 * `q` - Quit application
 * `Tab` / `Shift+Tab` - Navigate between tabs
 * `s` - Open settings
 * `d` - Go to dashboard
Dashboard
 * `r` - Start scanning
 * `o` - Start organizing
 * `f` - Search files
 * `u` - update target/destination folder stats
Settings
 * `↑`/`↓` - Navigate settings
 * `Enter` - Edit setting
 * `Space` - Toggle checkbox
 * `S` - Save settings
 * `R` - Reset to defaults

## 🛠️ Configuration
VisualVault stores its configuration in:

 * Linux: ~/.config/visualvault/config.toml
 * macOS: ~/Library/Application Support/visualvault/config.toml
 * Windows: %APPDATA%\visualvault\config.toml
Example Configuration
```toml
source_folder = "/home/mikko/dev/visualvault/testing"
destination_folder = "/home/mikko/dev/visualvault/testing/images"
recurse_subfolders = true
verbose_output = true
organize_by = "monthly"
separate_videos = false
dry_run = false
keep_original_structure = false
rename_duplicates = true
lowercase_extensions = true
preserve_metadata = true
create_thumbnails = false
worker_threads = 8
buffer_size = 8388608
enable_cache = true
parallel_processing = true
skip_hidden_files = false
optimize_for_ssd = false
```
## 📂 Organization Modes
 * Yearly: 2024/image.jpg
 * Monthly: 2024/03-March/image.jpg
 * Daily: 2024/03/15/image.jpg
 * By Type: Images/image.jpg
 * Type + Date: Images/2024/03-March/image.jpg
##  🤝 Contributing
Contributions are welcome! Please feel free to submit a Pull Request. For major changes, please open an issue first to discuss what you would like to change.

Development Setup
## 📝 Roadmap
 * <input disabled="" type="checkbox"> Complete duplicate file handling UI
 * <input disabled="" type="checkbox"> Add video metadata extraction
 * <input disabled="" type="checkbox"> Support filtering
 * <input disabled="" type="checkbox"> Add export/import functionality
 * <input disabled="" type="checkbox"> Cloud storage integration

## 📄 License
This project is licensed under the MIT License - see the LICENSE file for details.

## 🙏 Acknowledgments
 * built with Ratatui - Terminal UI framework
 * Uses Tokio - Async runtime for Rust
 * walkdir - Recursive directory traversal
 * kamadak-exif - EXIF metadata extraction
<p align="center"> Written with ❤️ in Rust & built with Ratatui </p>