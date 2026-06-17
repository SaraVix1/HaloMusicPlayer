# Halo Music Player

A desktop music player built with Tauri v2 (Rust + React/TypeScript), SQLite, and local audio playback.

## Tech Stack

- **Framework**: Tauri v2
- **Frontend**: React + TypeScript + shadcn/ui + Tailwind CSS v4
- **Backend**: Rust — `rodio` (audio), `lofty` (metadata), `rusqlite` (database)

## Prerequisites

- Rust (stable MSVC toolchain: `stable-x86_64-pc-windows-msvc`)
- Visual Studio with the "Desktop development with C++" workload
- Node.js ≥ 18

## Getting Started

```bash
npm install
npm run tauri dev
```

## IDE Setup

VS Code + [Tauri](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode) + [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer)
