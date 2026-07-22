<h1 style="font-family: Arial, sans-serif; font-size: 36px; color: #7C5CFC; display: flex; align-items: center; gap: 12px; border-bottom: 3px solid #7C5CFC; padding-bottom: 8px;">
  Hive — Private Art Collection
</h1>

Hive is a local-first desktop gallery for discovering, organizing, and saving contemporary artwork. It is built with **Tauri, React, TypeScript, Tailwind CSS, and SQLite**.

---

## Tech Used

![Tauri](https://img.shields.io/badge/Tauri-24C8DB?style=for-the-badge&logo=tauri&logoColor=white)
![React](https://img.shields.io/badge/React-61DAFB?style=for-the-badge&logo=react&logoColor=black)
![TypeScript](https://img.shields.io/badge/TypeScript-3178C6?style=for-the-badge&logo=typescript&logoColor=white)
![Vite](https://img.shields.io/badge/Vite-646CFF?style=for-the-badge&logo=vite&logoColor=white)
![Tailwind CSS](https://img.shields.io/badge/Tailwind_CSS-06B6D4?style=for-the-badge&logo=tailwindcss&logoColor=white)
![Rust](https://img.shields.io/badge/Rust-000000?style=for-the-badge&logo=rust&logoColor=white)
![pnpm](https://img.shields.io/badge/pnpm-F69220?style=for-the-badge&logo=pnpm&logoColor=white)

---

## Features

- Curated discovery view with featured exhibitions and new arrivals
- Browseable collection, artists, and saved-artwork views
- Artwork search and save actions
- Light and dark visual themes
- Custom desktop titlebar with native window controls
- Local SQLite database preloaded as `hive.db`
- Native integrations for dialogs, files, notifications, storage, logging, clipboard, HTTP, and window-state restoration

---

## Screenshots

<img src="screenshots/home.png" alt="Hive discovery view" width="88%"/>

**Discover:** Featured exhibition, artwork search, and a curated selection of new arrivals.

---

<img src="screenshots/collection.png" alt="Hive collection view" width="88%"/>

**Collection:** A browsable private archive for reviewing and organizing artwork.

---

<img src="screenshots/artists.png" alt="Hive artists view" width="88%"/>

**Artists:** A focused directory for discovering work by artist.

---

## Project Structure

```text
src/
|-- components/             # Gallery cards, layout, titlebar, and UI primitives
|-- config/                 # App and route configuration
|-- data/                   # Artwork and artist data
|-- hooks/                  # Gallery state, theme, and window controls
|-- pages/                  # Discover, Collection, Artists, Saved, Settings
|-- router/                 # Hash-router configuration
|-- sections/               # Composed gallery sections
|-- styles/                 # Global Tailwind styling
|-- types/                  # Shared gallery types
|-- App.tsx                 # Application shell
`-- main.tsx                # React entry point

src-tauri/
|-- src/                    # Rust application entry point
|-- capabilities/           # Tauri permission definitions
|-- tauri.conf.json         # Window, build, SQLite, and bundle configuration
`-- Cargo.toml              # Rust dependencies
```

---

## Getting Started

### Prerequisites

- Node.js 18+
- pnpm
- Rust toolchain
- Tauri system dependencies for your operating system

### Install dependencies

```bash
pnpm install
```

### Run in the browser

```bash
pnpm dev
```

The Vite server is configured for `http://localhost:61427`.

### Run the desktop app

```bash
pnpm tauri dev
```

---

## Available Scripts

```bash
pnpm dev          # Start Vite
pnpm build        # Type-check and build frontend assets
pnpm typecheck    # Run TypeScript checking
pnpm rust:check   # Check the Rust/Tauri application
pnpm preview      # Preview the production frontend build
pnpm tauri dev    # Run the desktop app
```

---

## Core Pages

- `#/` — Discover
- `#/collection` — Artwork collection
- `#/artists` — Artist directory
- `#/saved` — Saved artwork
- `#/settings` — Gallery settings

---

## Collection Workflow

1. Start in **Discover** to browse the current exhibition and selected arrivals.
2. Use the search control to find artwork and artists.
3. Open an artwork to review it, then save it to keep it in the private collection.
4. Move between collection, artist, and saved views using the desktop sidebar.

---

## Data and Desktop Behavior

- Hive preloads `sqlite:hive.db` through the Tauri SQL plugin.
- The frontend uses a desktop-safe hash router, so navigation remains reliable inside the native window.
- Theme preference and other lightweight preferences use Tauri Store.
- The desktop surface includes persistent window state, clipboard, dialog, file, notification, HTTP, and logging integrations.
