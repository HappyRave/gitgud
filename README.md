# GitGud - Git Repository Manager

A modern desktop Git UI for managing multiple repositories, built with Tauri and SvelteKit.

## Features

✨ **MVP Features**
- 📁 Manage multiple Git repositories from one interface
- 📊 View repository status with visual indicators
- 🔄 Stage/unstage files with a click
- 💬 Commit changes with a message
- ⬆️ Push changes to remote
- ⬇️ Pull updates from remote
- 🌿 View and switch branches
- 📜 Browse commit history
- 🎨 Modern dark theme interface

## Getting Started

### Prerequisites
- Node.js (v20+ recommended)
- Rust (latest stable)
- npm or yarn

### Development

1. **Install dependencies:**
   ```sh
   npm install --force
   ```

2. **Run in development mode:**
   ```sh
   npm run tauri:dev
   ```

3. **Build for production:**
   ```sh
   npm run tauri:build
   ```

## Usage

### Adding a Repository
1. Click the `+` button in the sidebar
2. Select a folder containing a Git repository
3. The repository will appear in the sidebar with status indicators

### Status Indicators
- ✓ (green) - Clean, no changes
- ● (yellow) - Uncommitted changes
- ↑ (blue) - Commits ahead of remote
- ↓ (orange) - Commits behind remote

### Making Commits
1. Select a repository from the sidebar
2. Stage files by clicking the `+` button next to unstaged files
3. Click the "Commit" button in the header
4. Enter a commit message and click "Commit"

### Syncing with Remote
- **Pull**: Click the "Pull" button to fetch and merge changes
- **Push**: Click the "Push" button to push your commits

### Switching Branches
1. Go to the "Branches" tab
2. Click "Checkout" next to the branch you want to switch to

## Tech Stack

- **Frontend**: SvelteKit 2.x with TypeScript
- **Backend**: Rust with Tauri 2.x
- **Git Operations**: git2-rs (libgit2 Rust bindings)
- **UI**: Custom CSS with VS Code-inspired dark theme

## Known Limitations (MVP)

- Merge conflicts require manual resolution
- Complex merge operations not yet supported
- No diff viewer (yet)
- No stash support (yet)
- Authentication for HTTPS repos may require credential manager setup

## License

MIT

