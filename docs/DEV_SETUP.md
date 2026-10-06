# Saeed Desktop development setup

## Requirements

- Windows 10/11
- Node.js 22
- Rust stable
- Tauri CLI 2.12.1 through the project dependency

## Install

```powershell
npm ci
```

If the repository is being bootstrapped without a lockfile, generate it once with:

```powershell
npm install --package-lock-only
```

Commit the generated `package-lock.json` before using `npm ci`.

## Checks

Run these before pushing:

```powershell
cd src-tauri
cargo fmt --all
cargo check
cargo clippy --all-targets --all-features -- -D warnings
cd ..
npm run check
npx prettier --check "src/**/*.{ts,css,json}" "src-tauri/**/*.{rs,json}" package.json
```

## Build

```powershell
npm run icons
npm run tauri build
```

The Windows NSIS installer is written below `src-tauri/target/release/bundle/nsis/`.

Do not commit generated build output, `node_modules`, `dist`, or generated Tauri icon/gen directories.
