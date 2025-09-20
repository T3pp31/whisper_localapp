Place Whisper ggml model files in this directory before running `cargo tauri build` to include them in the installer.

Examples:
- ggml-large-v3-turbo-q5_0.bin
- ggml-base.bin

Note: At least one non-hidden file must exist here for the build to succeed when `tauri.conf.json` lists `models/**` as bundle resources.
