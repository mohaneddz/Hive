# Hive

A clean, future-ready Tauri desktop workspace built with React, TypeScript,
Tailwind CSS, and Vite.

## Commands

```bash
pnpm dev          # Run the frontend
pnpm build        # Type-check and build the frontend
pnpm typecheck    # TypeScript only
pnpm rust:check   # Validate the Rust/Tauri application
pnpm tauri dev    # Run the desktop app
```

## Source structure

- `components/` - reusable UI and layout primitives
- `sections/` - page sections composed from components
- `pages/` - route-level views
- `data/` - dummy data and asynchronous data getters
- `hooks/` - reusable React behavior
- `config/` - app and routing configuration
- `utils/` - framework-independent helpers
- `styles/` - global Tailwind theme and platform styling
- `types/` - shared domain types
- `router/` - desktop-safe hash router configuration

The Tauri backend is under `src-tauri/`. Its main-window capability set includes
window controls, file access, dialogs, HTTP, notifications, persistent storage,
window-state restoration, clipboard access, logging, and SQLite.
