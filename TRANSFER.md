# Aurora Viewer sandbox transfer

This source snapshot follows the official v0.9.10 release baseline and includes
the sandbox's base-outfit experiment, localization updates and project-specific
AGENTS.md instructions. The persistent Git branch is sandbox/base-outfits in
the Darque/Aurora-Viewer repository.

For a complete Git checkout on the new machine:

    git clone --branch sandbox/base-outfits https://github.com/Darque/Aurora-Viewer.git Aurora-Viewer
    cd Aurora-Viewer
    git remote add upstream https://github.com/Aurora-Viewer/Aurora-Viewer.git

Then read AGENTS.md and build with the toolchain in rust-toolchain.toml:

    cargo build -p aurora-viewer

The ZIP is a source snapshot. It excludes Git metadata, build outputs, caches,
temporary captures and logs; clone the branch above for Git history and future
pushes. No credentials belong in this archive.

The source built successfully on macOS arm64 during packaging. On that host,
the offline demo process became unresponsive before showing a window; retest
the demo on the new machine before relying on visual validation.
