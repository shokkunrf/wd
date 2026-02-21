# wd

git worktree manager with devcontainer support.

bare clone + git worktree でリポジトリを管理するPOSIX shスクリプト。ghqライクなディレクトリ配置で、devcontainerとの統合を自動化します。

## Install

```sh
curl -fsSL https://raw.githubusercontent.com/shokkunrf/wd/main/install.sh | sh
```

`WD_INSTALL_DIR`でインストール先を変更できます（デフォルト:`/usr/local/bin`）。

## Usage

```
wd <command> [options]

Project Management:
  clone <repo-url> [-b <branch>]    Clone repository (bare + worktree)
  list [--full-path] [--worktrees]  List managed projects

Worktree Management:
  add <branch> [-b]                 Add worktree (-b: create new branch)
  add --pr <number>                 Add PR review worktree
  remove <name>... [-b|--branch]    Remove worktree(s) and optionally branch
  remove -a [-b|--branch]           Remove all non-default worktrees

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

`.bashrc`/`.zshrc`に以下を追加すると、`cd`を伴う操作が便利になります。
`fzf`の代わりに`peco`等のセレクターに置き換えられます。

```sh
# fzf でワークツリーを選んで移動
wdc() { dir=$(wd list --full-path --worktrees | fzf) && cd "$dir"; }

# ワークツリー追加後に自動で移動
wda() { dir=$(wd add "$@") && cd "$dir"; }
```

## License

MIT
