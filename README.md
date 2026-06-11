# wd

CLI tool for managing git worktrees with devcontainer support.

A POSIX sh script that manages repositories with bare clone + git worktree. Uses a ghq-like directory layout and automates devcontainer integration.

## Install

```sh
curl -fsSL https://github.com/shokkunrf/wd/releases/latest/download/install.sh | sh
```

Set `WD_INSTALL_DIR` to change the install location (default: `/usr/local/bin`).

### Dev Container Feature

Add to your `devcontainer.json`:

```json
{
  "features": {
    "ghcr.io/shokkunrf/devcontainer-features/wd:latest": {}
  }
}
```

## Usage

```
wd <command> [options]

Project Management:
  clone <repo-url> [-b <branch>]    Clone repository (bare + worktree)
  list [--full-path] [--worktrees]  List managed projects

Worktree Management:
  add <branch>                      Add worktree from a branch or commit-ish
  add -b <branch> [<base>]          Add worktree, creating <branch> from <base>
  add --pr <number>                 Add PR review worktree
  remove <name>... [-b|--branch]    Remove worktree(s) and optionally branch
  remove -a [-b|--branch]           Remove all non-default worktrees
  repair                            Repair worktree relative paths

General:
  update                            Update wd to the latest version

Options:
  --version    Show version
  --help       Show this help

Environment:
  WD_ROOT    Root directory (default: ~/Repositories)
```

## Directory structure

```
$WD_ROOT/
└── github.com/
    └── owner/
        └── repo/
            ├── .bare/                              # bare git database
            ├── .git                                # pointer file
            ├── .devcontainer -> main/.devcontainer  # symlink
            ├── main/                               # default worktree
            ├── wt-feature/                         # wd add
            └── pr-123/                             # wd add --pr
```

## Tips

Add the following to `.bashrc`/`.zshrc` for convenient `cd` integration.
Replace `fzf` with `peco` or any selector of your choice.

```sh
# Select a worktree with fzf and cd into it
wds() { dir=$(wd list | fzf) && cd "${WD_ROOT:-$HOME/Repositories}/$dir"; }

# Add a worktree and cd into it
wda() { dir=$(wd add "$@") && cd "$dir"; }
```

## License

MIT
