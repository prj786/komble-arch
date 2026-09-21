// Curated "Popular" lists for the Discover screen's first-run experience.
//
// A new machine should be able to reach a browser, an editor and a language
// toolchain without searching the web — so this file is a short, ordered set
// of categories, each naming a handful of well-known apps. It is deliberately
// plain data (no Tauri, no stores) so `scripts/check-curated.mjs` can import
// it with plain Node and re-validate every package name against the real
// sources: the Arch repos (`pacman -Si`) and the AUR RPC.
//
// Source order is the promise, not a style choice:
//   "repo"      — the official Arch repositories (fast, signed, updates with
//                 the system). Always the first choice.
//   "aur"       — the AUR, `-bin` packages where building from source is slow.
//                 These are gated behind the PKGBUILD review like every AUR
//                 install in Komble.
//   "appimage"  — Komble's AM catalog (the AM slug as `pkg`), only for apps
//                 whose upstream ships an AppImage. `toItem` builds the item
//                 AppDetail and installFromItem expect from it.
//
// An APP's optional `icon` is the slug of its artwork in the AM catalog's icon
// set (the same files Komble shows for AppImages) — a repository or AUR app
// then wears its own icon instead of the package glyph; command-line tools
// have none on purpose. check-curated.mjs verifies every one resolves.
// A CATEGORY's `icon` is a glyph name from components/ui/icons.js — the same names the
// shell's Theme.qml uses, so the category chips share one icon language with
// the rest of the desktop. `desc` is one short, plain line (no marketing);
// `note` is an optional second, even shorter line for the card.
//
// Apps ewe already ships by default (nemo, kitty, zed, mpv, imv, zathura,
// helium-browser-bin, mise) may appear but never dominate a category.

// Same icon URL pattern as the backend's catalog.rs (`AM_ICON_URL`): a missing
// file just 404s and the card falls back to its glyph/initial (AppCard's
// on:error path), so it is safe to construct rather than fetch.
const AM_ICON = "https://raw.githubusercontent.com/Portable-Linux-Apps/Portable-Linux-Apps.github.io/main/icons/";

export const curatedCategories = [
  {
    id: "browsers",
    title: "Browsers",
    blurb: "Pick a web browser — every major engine, from Firefox to Chromium.",
    icon: "compass",
    apps: [
      { name: "Firefox", pkg: "firefox", source: "repo", icon: "firefox", desc: "The privacy-first browser from Mozilla." },
      { name: "Chromium", pkg: "chromium", source: "repo", icon: "chromium", desc: "The open-source browser Google Chrome is built on." },
      { name: "LibreWolf", pkg: "librewolf", source: "repo", icon: "librewolf", desc: "Firefox with privacy settings turned up by default." },
      { name: "Vivaldi", pkg: "vivaldi", source: "repo", icon: "vivaldi-stable", desc: "A Chromium browser built for power users." },
      { name: "Brave", pkg: "brave-bin", source: "aur", icon: "brave", desc: "Chromium with ad and tracker blocking built in." },
      { name: "Firefox Developer Edition", pkg: "firefox-developer-edition", source: "repo", desc: "Firefox for web developers, with the newest tooling." },
      { name: "qutebrowser", pkg: "qutebrowser", source: "repo", desc: "A keyboard-driven browser for Vim users." },
      { name: "Helium", pkg: "helium-browser-bin", icon: "helium", source: "repo", desc: "A lean Chromium browser without Google services.", note: "Ships with ewe." },
      { name: "Falkon", pkg: "falkon", source: "repo", icon: "falkon", desc: "The lightweight KDE browser." }
    ]
  },
  {
    id: "editors",
    title: "Code editors and IDEs",
    blurb: "Where you write code — from small editors to full IDEs.",
    icon: "fileCode",
    apps: [
      { name: "Visual Studio Code", pkg: "visual-studio-code-bin", source: "aur", icon: "visual-studio-code", desc: "Microsoft's editor, from the AUR." },
      { name: "VSCodium", pkg: "vscodium-bin", source: "aur", icon: "vscodium", desc: "VS Code without Microsoft's telemetry." },
      { name: "Zed", pkg: "zed", source: "repo", icon: "zed", desc: "A fast editor written in Rust.", note: "Ships with ewe." },
      { name: "Neovim", pkg: "neovim", source: "repo", icon: "nvim", desc: "The extensible Vim-based editor." },
      { name: "Vim", pkg: "vim", source: "repo", desc: "The classic modal text editor." },
      { name: "Emacs", pkg: "emacs", source: "repo", icon: "emacs", desc: "The extensible, self-documenting editor." },
      { name: "Helix", pkg: "helix", source: "repo", icon: "helix", desc: "A modal editor with built-in language support." },
      { name: "Sublime Text", pkg: "sublime-text-4", source: "aur", desc: "A fast editor with a large plugin ecosystem." },
      { name: "IntelliJ IDEA Community", pkg: "intellij-idea-community-edition", source: "repo", desc: "JetBrains' Java IDE, community edition." },
      { name: "PyCharm Community", pkg: "pycharm-community-edition", icon: "pycharm", source: "repo", desc: "The JetBrains IDE for Python." },
      { name: "JetBrains Toolbox", pkg: "jetbrains-toolbox", source: "aur", desc: "Installs and updates every JetBrains IDE." },
      { name: "Android Studio", pkg: "android-studio", icon: "android-studio", source: "aur", desc: "Google's IDE for Android apps." }
    ]
  },
  {
    id: "languages",
    title: "Languages and runtimes",
    blurb: "A language and its runtime — most can also be installed through mise.",
    icon: "terminal",
    apps: [
      { name: "mise", pkg: "mise", source: "repo", desc: "One tool that installs and switches language versions.", note: "Ships with ewe — the recommended way to manage runtimes." },
      { name: "Rust", pkg: "rustup", source: "repo", desc: "The Rust toolchain installer." },
      { name: "Go", pkg: "go", source: "repo", desc: "Google's compiled language and toolchain." },
      { name: "Node.js", pkg: "nodejs", source: "repo", icon: "node", desc: "JavaScript outside the browser.", note: "npm comes with it." },
      { name: "Python", pkg: "python", source: "repo", icon: "python", desc: "The Python interpreter.", note: "Add python-pip for pip." },
      { name: "OpenJDK", pkg: "jdk-openjdk", source: "repo", desc: "The Java development kit." },
      { name: ".NET SDK", pkg: "dotnet-sdk", source: "repo", desc: "Build C# and .NET applications." },
      { name: "PHP", pkg: "php", source: "repo", desc: "The PHP scripting language." },
      { name: "Ruby", pkg: "ruby", source: "repo", desc: "The Ruby language and runtime." },
      { name: "Zig", pkg: "zig", source: "repo", desc: "A low-level language and toolchain." },
      { name: "GCC", pkg: "gcc", source: "repo", desc: "The GNU C and C++ compiler." },
      { name: "Clang", pkg: "clang", source: "repo", desc: "The LLVM C and C++ compiler." }
    ]
  },
  {
    id: "devtools",
    title: "Developer tools",
    blurb: "Git, containers, databases, API clients and terminals.",
    icon: "git",
    apps: [
      { name: "Git", pkg: "git", source: "repo", desc: "The version control system." },
      { name: "GitHub CLI", pkg: "github-cli", source: "repo", desc: "Work with GitHub from the terminal." },
      { name: "Docker", pkg: "docker", source: "repo", desc: "Run software in containers." },
      { name: "Docker Compose", pkg: "docker-compose", source: "repo", desc: "Define and run multi-container apps." },
      { name: "Podman", pkg: "podman", source: "repo", icon: "podman", desc: "Rootless containers, as a drop-in for Docker." },
      { name: "lazygit", pkg: "lazygit", source: "repo", icon: "lazygit", desc: "A terminal UI for Git." },
      { name: "DBeaver", pkg: "dbeaver", source: "repo", icon: "dbeaver-ce", desc: "A database GUI for many engines." },
      { name: "DB Browser for SQLite", pkg: "sqlitebrowser", source: "repo", icon: "sqlitebrowser", desc: "Create and browse SQLite databases." },
      { name: "Postman", pkg: "postman-bin", source: "aur", icon: "postman", desc: "Build and test HTTP APIs." },
      { name: "Bruno", pkg: "bruno-bin", source: "aur", icon: "bruno", desc: "An open-source API client." },
      { name: "Alacritty", pkg: "alacritty", source: "repo", icon: "alacritty", desc: "A fast terminal with GPU rendering." },
      { name: "WezTerm", pkg: "wezterm", source: "repo", icon: "wezterm", desc: "A GPU-accelerated terminal in Rust." }
    ]
  },
  {
    id: "ai",
    title: "AI tools",
    blurb: "Local AI — chat, coding helpers and speech models that run on your machine.",
    icon: "sparkles",
    apps: [
      { name: "Ollama", pkg: "ollama", source: "repo", icon: "ollama", desc: "Run large language models locally." },
      { name: "LM Studio", pkg: "lmstudio", source: "appimage", desc: "Download and chat with local models." },
      { name: "Open WebUI", pkg: "open-webui", source: "aur", icon: "open-webui", desc: "A web chat interface for local models." },
      { name: "Jan", pkg: "jan", source: "aur", icon: "jan", desc: "An open-source local AI assistant." },
      { name: "GPT4All", pkg: "gpt4all-chat", source: "aur", desc: "Run open models on your own hardware." },
      { name: "Aider", pkg: "aider-chat", source: "aur", desc: "AI pair programming in the terminal." },
      { name: "whisper.cpp", pkg: "whisper-cpp", source: "repo", desc: "Speech-to-text that runs locally." }
    ]
  },
  {
    id: "communication",
    title: "Communication",
    blurb: "Chat, calls and email.",
    icon: "user",
    apps: [
      { name: "Discord", pkg: "discord", source: "repo", icon: "discord", desc: "Chat and voice for communities." },
      { name: "Telegram", pkg: "telegram-desktop", source: "repo", icon: "telegram", desc: "Fast, cloud-synced messaging." },
      { name: "Signal", pkg: "signal-desktop", source: "repo", icon: "signal", desc: "End-to-end encrypted messaging." },
      { name: "Element", pkg: "element-desktop", source: "repo", icon: "element-desktop", desc: "A Matrix chat client." },
      { name: "Thunderbird", pkg: "thunderbird", source: "repo", icon: "thunderbird", desc: "Email, calendars and contacts." },
      { name: "Slack", pkg: "slack-desktop", source: "aur", icon: "slack", desc: "Team chat and channels." },
      { name: "Zoom", pkg: "zoom", source: "aur", icon: "zoom", desc: "Video meetings." },
      { name: "Vesktop", pkg: "vesktop", source: "aur", icon: "vesktop", desc: "A customisable Discord client." },
      { name: "Jitsi Meet", pkg: "jitsi-meet-desktop", source: "aur", icon: "jitsi-meet", desc: "Encrypted video calls." }
    ]
  },
  {
    id: "office",
    title: "Office and notes",
    blurb: "Documents, notes and e-books.",
    icon: "book",
    apps: [
      { name: "LibreOffice", pkg: "libreoffice-fresh", source: "repo", icon: "libreoffice", desc: "Word, spreadsheets and more." },
      { name: "ONLYOFFICE", pkg: "onlyoffice-bin", source: "aur", icon: "onlyoffice", desc: "An office suite with strong format support." },
      { name: "Obsidian", pkg: "obsidian", source: "repo", icon: "obsidian", desc: "Plain-text notes linked together." },
      { name: "Joplin", pkg: "joplin-desktop", source: "aur", icon: "joplin", desc: "Notes you can sync yourself." },
      { name: "Logseq", pkg: "logseq", source: "appimage", desc: "An outliner for notes and tasks." },
      { name: "Zettlr", pkg: "zettlr", source: "repo", icon: "zettlr", desc: "Markdown writing for academics." },
      { name: "Calibre", pkg: "calibre", source: "repo", icon: "calibre", desc: "Organise and convert e-books." },
      { name: "Okular", pkg: "okular", source: "repo", desc: "A document and PDF viewer." },
      { name: "Evince", pkg: "evince", source: "repo", icon: "evince", desc: "A simple PDF reader." }
    ]
  },
  {
    id: "media",
    title: "Media",
    blurb: "Players, editors and streaming.",
    icon: "eye",
    apps: [
      { name: "VLC", pkg: "vlc", source: "repo", icon: "vlc", desc: "The media player that plays everything." },
      { name: "mpv", pkg: "mpv", source: "repo", icon: "mpv", desc: "A minimal, scriptable video player.", note: "Ships with ewe." },
      { name: "Kodi", pkg: "kodi", source: "repo", desc: "A media centre for the living room." },
      { name: "Strawberry", pkg: "strawberry", source: "repo", icon: "strawberry", desc: "A music player and collection organiser." },
      { name: "Spotify", pkg: "spotify", source: "aur", icon: "spotify", desc: "Music streaming." },
      { name: "Kdenlive", pkg: "kdenlive", source: "repo", icon: "kdenlive", desc: "A full-featured video editor." },
      { name: "OBS Studio", pkg: "obs-studio", source: "repo", icon: "obs-studio", desc: "Record and stream your screen." },
      { name: "Shotcut", pkg: "shotcut", source: "repo", icon: "shotcut", desc: "A straightforward video editor." },
      { name: "HandBrake", pkg: "handbrake", source: "repo", icon: "handbrake", desc: "Convert and compress video." },
      { name: "FreeTube", pkg: "freetube-bin", source: "aur", icon: "freetube", desc: "Watch YouTube without the tracking." }
    ]
  },
  {
    id: "graphics",
    title: "Graphics and design",
    blurb: "Image, photo and 3D tools.",
    icon: "sliders",
    apps: [
      { name: "GIMP", pkg: "gimp", source: "repo", icon: "gimp", desc: "A full image editor." },
      { name: "Inkscape", pkg: "inkscape", source: "repo", icon: "inkscape", desc: "Vector graphics and illustration." },
      { name: "Krita", pkg: "krita", source: "repo", icon: "krita", desc: "Digital painting." },
      { name: "Blender", pkg: "blender", source: "repo", icon: "blender", desc: "3D modelling, animation and rendering." },
      { name: "darktable", pkg: "darktable", source: "repo", icon: "darktable", desc: "Develop RAW photos." },
      { name: "RawTherapee", pkg: "rawtherapee", source: "repo", icon: "rawtherapee", desc: "Another RAW photo developer." },
      { name: "Pinta", pkg: "pinta", source: "repo", icon: "pinta", desc: "A simple drawing and editing tool." },
      { name: "MyPaint", pkg: "mypaint", source: "repo", icon: "mypaint", desc: "Painting with a focus on brushes." },
      { name: "Aseprite", pkg: "aseprite", source: "aur", desc: "Pixel-art animation." },
      { name: "draw.io", pkg: "drawio-desktop", source: "repo", icon: "draw.io", desc: "Diagrams and flowcharts." }
    ]
  },
  {
    id: "games",
    title: "Games",
    blurb: "Launchers, emulators and open-source games.",
    icon: "puzzle",
    apps: [
      { name: "Steam", pkg: "steam", source: "repo", note: "Needs the multilib repository.", icon: "steam", desc: "Valve's store and launcher." },
      { name: "Lutris", pkg: "lutris", source: "repo", icon: "lutris", desc: "Run games from many stores in one place." },
      { name: "Heroic", pkg: "heroic-games-launcher", source: "aur", icon: "heroic-games-launcher", desc: "Epic and GOG games on Linux." },
      { name: "Bottles", pkg: "bottles", source: "aur", icon: "bottles", desc: "Run Windows apps in Wine easily." },
      { name: "Prism Launcher", pkg: "prismlauncher", source: "repo", icon: "prismlauncher", desc: "Manage Minecraft instances." },
      { name: "Luanti", pkg: "luanti", source: "repo", icon: "luanti", desc: "The open-source voxel game, formerly Minetest." },
      { name: "0 A.D.", pkg: "0ad", source: "repo", icon: "0ad", desc: "A free real-time strategy game." },
      { name: "RetroArch", pkg: "retroarch", source: "repo", icon: "retroarch", desc: "Emulators for many consoles in one." },
      { name: "Dolphin", pkg: "dolphin-emu", source: "repo", icon: "dolphin-emu", desc: "A GameCube and Wii emulator." },
      { name: "PCSX2", pkg: "pcsx2", source: "aur", icon: "pcsx2", desc: "A PlayStation 2 emulator." },
      { name: "ProtonUp-Qt", pkg: "protonup-qt", source: "aur", icon: "protonup-qt", desc: "Install Proton and Wine versions for Steam." }
    ]
  },
  {
    id: "system",
    title: "System and utilities",
    blurb: "Disk, backup and system tools.",
    icon: "hardDrive",
    apps: [
      { name: "btop", pkg: "btop", source: "repo", icon: "btop", desc: "A resource monitor for the terminal." },
      { name: "htop", pkg: "htop", source: "repo", icon: "htop", desc: "Watch processes and memory." },
      { name: "fastfetch", pkg: "fastfetch", source: "repo", icon: "fastfetch", desc: "Show your system at a glance." },
      { name: "GNOME Disks", pkg: "gnome-disk-utility", source: "repo", desc: "Format and manage drives." },
      { name: "GParted", pkg: "gparted", source: "repo", desc: "Resize and move partitions." },
      { name: "Filelight", pkg: "filelight", source: "repo", icon: "filelight", desc: "See what is using your disk space." },
      { name: "BleachBit", pkg: "bleachbit", source: "repo", icon: "bleachbit", desc: "Clear caches and junk files." },
      { name: "Timeshift", pkg: "timeshift", source: "repo", desc: "System snapshots you can restore." },
      { name: "Virt-Manager", pkg: "virt-manager", source: "repo", icon: "virt-manager", desc: "Manage virtual machines." },
      { name: "Wine", pkg: "wine", source: "repo", icon: "wine", desc: "Run Windows programs." }
    ]
  }
];

/**
 * Build the item shape AppDetail and installFromItem expect from one curated
 * app. Repo/AUR entries become `pkg` items (section "repo"/"aur"); AppImage
 * entries become AM-catalog items (source "am", the AM slug as `id`).
 */
export function toItem(app) {
  if (app.source === "appimage") {
    return {
      kind: "appimage",
      id: app.pkg,
      name: app.name,
      description: app.desc,
      plainDesc: app.desc,
      source: "am",
      icon: `${AM_ICON}${app.pkg}.png`,
      categories: []
    };
  }
  return {
    kind: "pkg",
    id: `pkg:${app.pkg}`,
    pkg: app.pkg,
    name: app.name,
    description: app.desc,
    plainDesc: app.desc,
    section: app.source === "aur" ? "aur" : "repo",
    icon: app.icon ? `${AM_ICON}${app.icon}.png` : "",
    version: "",
    categories: [],
    installed: false
  };
}
