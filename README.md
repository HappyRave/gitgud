# GitGud - Modern Git Client

A modern desktop Git client with a clean, native interface. Built with Tauri and SvelteKit for fast, efficient repository management.

## ✨ Features

### 🎨 **Interface**
- **Dark/Light Theme** - Automatic system theme detection with manual toggle
- **Custom Title Bar** - Native window controls with integrated theme switcher
- **Split Diff View** - Side-by-side diff viewer with line numbers (like SourceTree)
- **Bottom Commit Bar** - Quick, always-accessible commit interface

### 📁 **Repository Management**
- **Multiple Repos** - Manage all your repositories from one interface
- **Clone & Init** - Clone from URLs or initialize new repositories
- **Open Existing** - Add existing local repositories
- **Real-time Status** - Visual indicators for changes, ahead/behind counts

### 📝 **File Operations**
- **Multi-Select** - Ctrl+click (toggle) and Shift+click (range) support
- **Stage/Unstage** - Individual files or batch operations
- **Stage All** - Quick stage all changes button
- **Visual File Status** - Color-coded status badges (new, modified, deleted)

### 💾 **Commits & Push/Pull**
- **Quick Commit** - Bottom bar with Enter key shortcut
- **Commit & Push** - One-click commit and push workflow
- **First Commit Support** - Handles new repositories with no commits
- **Pull Operations** - Fetch and merge from remote

### 🔐 **Authentication**
- **Credential Storage** - Securely saves GitHub credentials (Personal Access Tokens)
- **HTTPS Support** - Automatic credential prompting for authenticated push/pull
- **SSH Agent Support** - Falls back to SSH agent for key-based auth
- **Auto-retry** - Uses saved credentials automatically on subsequent pushes

### 🔄 **Remote Management**
- **Add Remotes** - Configure remote repositories with custom names
- **Remote Detection** - Automatic detection and helpful prompts
- **Push with Tracking** - Sets upstream branch on first push

### 📊 **Diff Viewer**
- **Two-Column Line Numbers** - Old and new line numbers like SourceTree
- **Color-Coded Changes** - Green for additions, red for deletions
- **Hunk Headers** - Sticky section headers while scrolling
- **Context Lines** - Shows surrounding unchanged code

### 🌿 **Branches & History**
- **Branch List** - View local and remote branches
- **Checkout Branches** - Switch branches with one click
- **Commit History** - Browse commits with author, date, and message
- **Ahead/Behind Indicators** - Visual sync status with remote

### ⚙️ **Settings**
- **Git Version Info** - View installed Git version and path
- **System Information** - Platform and environment details
- **Application Info** - Version and tech stack information

## 🚀 Getting Started

### Prerequisites
- **Node.js** v20+ (v24+ recommended)
- **Rust** 1.92.0+
- **Git** installed and accessible in PATH
- **npm** or yarn

### Installation

1. **Clone the repository:**
   ```sh
   git clone https://github.com/yourusername/gitgud.git
   cd gitgud
   ```

2. **Install dependencies:**
   ```sh
   npm install
   ```

3. **Run in development mode:**
   ```sh
   npm run tauri:dev
   ```

4. **Build for production:**
   ```sh
   npm run tauri:build
   ```

## 📖 Usage Guide

### Adding Repositories

**Clone a Repository:**
1. Click the `+` button in the sidebar
2. Select "Clone Repository"
3. Enter the repository URL
4. Choose a destination folder

**Initialize New Repository:**
1. Click the `+` button
2. Select "Initialize Repository"
3. Choose or create a folder

**Open Existing:**
1. Click the `+` button
2. Select "Open Repository"
3. Navigate to a folder with a `.git` directory

### Making Commits

1. **Stage your changes** - Click files to stage them individually or use "Stage All"
2. **Review diff** - Click a file to see the detailed diff with line numbers
3. **Write commit message** - Type in the bottom bar (appears when files are staged)
4. **Commit** - Click "Commit" or "Commit & Push" button
   - Press **Enter** in the message box for quick commit

### Working with Remotes

**First Time Setup:**
1. If no remote is configured, a banner will prompt you
2. Click "Add Remote" and enter the remote URL
3. Common name is "origin"

**Pushing Changes:**
1. Click "Push" in the header or use "Commit & Push"
2. For HTTPS repos, enter your GitHub username and Personal Access Token when prompted
3. Credentials are saved for future pushes

**Creating a Personal Access Token:**
1. GitHub → Settings → Developer settings → Personal access tokens → Tokens (classic)
2. Generate new token with `repo` scope
3. Copy and paste when GitGud prompts for credentials

### Multi-Select Operations

- **Ctrl+Click** - Toggle individual files
- **Shift+Click** - Select range of files
- **Stage Selected** - Batch stage your selection

### Keyboard Shortcuts

- **Enter** - Commit (when message box is focused)
- **Esc** - Close dialogs
- **Ctrl+R** / **Cmd+R** - Refresh (when implemented)

## 🛠️ Tech Stack

- **Frontend**: SvelteKit 2.49.1 with Svelte 5.45.6 (runes mode)
- **Backend**: Rust 1.92.0 with Tauri 2.9.x
- **Git Operations**: git2-rs 0.19 (libgit2 bindings)
- **UI Components**: Custom components with shadcn-svelte styling
- **Styling**: Tailwind CSS v4.1.18 with CSS variables
- **Icons**: lucide-svelte
- **Secure Storage**: tauri-plugin-store for credentials

## 🎯 Roadmap

### Planned Features
- [ ] Stash operations (save, pop, apply)
- [ ] Cherry-pick commits
- [ ] Rebase operations
- [ ] Merge conflict resolution UI
- [ ] Tag management
- [ ] Submodule support
- [ ] Graph visualization for branches
- [ ] Search commits and files
- [ ] Blame view
- [ ] File history

### Under Consideration
- [ ] Light theme improvements
- [ ] Custom keyboard shortcuts
- [ ] Commit templates
- [ ] GPG commit signing
- [ ] Git LFS support
- [ ] Multiple account management

## 🐛 Known Issues

- Unborn branch handling (new repos) has edge cases
- Large diffs may have performance impact
- Merge conflict UI not yet implemented
- Some git operations may need terminal fallback

## 🤝 Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

## 📄 License

MIT

## 🙏 Acknowledgments

Built with:
- [Tauri](https://tauri.app/) - The Rust-powered app framework
- [SvelteKit](https://kit.svelte.dev/) - The Svelte framework
- [libgit2](https://libgit2.org/) - Git library used by git2-rs
- [shadcn-svelte](https://www.shadcn-svelte.com/) - UI component inspiration

