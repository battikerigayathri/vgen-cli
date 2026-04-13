# resmate-cli

ResMate platform CLI.

## Building for Windows from macOS

You can cross-compile the CLI to Windows (x64 or x86) on a Mac.

### One-time setup

```bash
# Rust Windows targets (GNU/MinGW)
rustup target add x86_64-pc-windows-gnu i686-pc-windows-gnu

# MinGW linker and runtime
brew install mingw-w64
```

### Build

```bash
# 64-bit Windows
./scripts/build-windows.sh --arch x64

# 32-bit Windows
./scripts/build-windows.sh --arch x86

# Debug build (optional)
./scripts/build-windows.sh --arch x64 --debug
```

Output:

- **x64:** `target/x86_64-pc-windows-gnu/release/resmate.exe`
- **x86:** `target/i686-pc-windows-gnu/release/resmate.exe`

Test the `.exe` on a real Windows machine (or use [Wine](https://www.winehq.org/) for a quick smoke test on macOS).
