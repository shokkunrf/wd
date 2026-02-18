# `wd` 技術設計書

**wd** (git **w**orktree for **d**evcontainer) — bare clone + git worktree によるリポジトリ管理 CLI

## 1. 目的と背景

### 1.1 解決する課題

| 課題                     | 現状                                               | wd による解決                                     |
| ------------------------ | -------------------------------------------------- | ------------------------------------------------- |
| リポジトリの構造化管理   | ghq を使うが worktree 非対応                       | ghq ライクな配置 + bare clone を統合              |
| ブランチ切り替えのコスト | `git switch` で作業中断・stash が必要              | worktree で並行作業、各ブランチが独立ディレクトリ |
| devcontainer との統合    | worktree 運用時に `.devcontainer` を手動で symlink | `wd clone` が自動で symlink を作成                |

### 1.2 設計原則

1. **Single-file** — 単一の POSIX sh スクリプト。ビルド不要でそのまま実行・配布
2. **最小依存** — git, POSIX sh のみ必須。外部ツール依存なし
3. **ghq 互換のディレクトリ配置** — `<root>/<host>/<owner>/<repo>` 構造
4. **devcontainer ファースト** — `.devcontainer` symlink を自動管理
5. **POSIX 互換** — `/bin/sh` で動作。bash, dash, zsh 等の POSIX 互換シェルで実行可能

## 2. 管理ディレクトリ構造

```
$WD_ROOT/                                  # デフォルト: ~/Repositories
└── <host>/                                # github.com, gitlab.com 等
    └── <owner>/
        └── <project>/                     # プロジェクトルート
            ├── .bare/                     # bare git データベース
            ├── .git                       # pointer file ("gitdir: ./.bare")
            ├── .devcontainer -> <default-branch>/.devcontainer  # symlink
            └── <default-branch>/          # デフォルトワークツリー
                ├── .devcontainer/
                │   └── devcontainer.json
                └── ...
```

### 2.1 プロジェクトの識別

- **プロジェクトルート**: `.bare/` ディレクトリを含むディレクトリ
- **検出**: カレントディレクトリから親方向へ `.bare/` を探索

## 3. コマンド仕様

### 3.1 `wd clone` — リポジトリ取得

**書式**: `wd clone <repo-url> [-b <branch>]`

**入力**: SSH URL のみ (`git@<host>:<owner>/<repo>[.git]`)

**処理フロー**:

```
1. URL を解析 → host/owner/repo を決定
2. $WD_ROOT/host/owner/repo が既存なら → エラー終了
3. mkdir -p でプロジェクトディレクトリを作成 (失敗時は trap で自動削除)
4. git clone --bare <url> .bare
5. echo "gitdir: ./.bare" > .git   (pointer file)
6. git config remote.origin.fetch "+refs/heads/*:refs/remotes/origin/*"
7. git fetch origin
8. デフォルトブランチを決定:
   a. -b 指定あり → そのブランチ
   b. 空リポジトリ (-b なし) → エラー
   c. refs/remotes/origin/HEAD から取得
   d. main → master の順にフォールバック
   e. いずれも見つからない → エラー (-b を案内)
9. 空リポジトリ → git worktree add --orphan -b <branch>
   通常 → git worktree add <branch> <branch>
10. <branch>/.devcontainer が存在すれば symlink を作成
11. プロジェクトディレクトリのパスを stdout に出力
```

## 4. 設定

| 変数      | デフォルト       | 説明                                 |
| --------- | ---------------- | ------------------------------------ |
| `WD_ROOT` | `~/Repositories` | プロジェクト配置のルートディレクトリ |

## 5. アーキテクチャ

### 5.1 ファイル構成

```
wd/
├── src/
│   └── wd.sh                 # 本体 (単一実行ファイル)
├── install.sh                # POSIX sh インストーラ
├── .github/workflows/
│   ├── ci.yml
│   └── release.yml
├── docs/                     # 設計ドキュメント
├── .editorconfig
├── .gitignore
├── LICENSE
└── README.md
```

`wd` ファイル内の構成:

```sh
#!/bin/sh
set -eu

WD_VERSION="dev"

# --- utils ---         共通関数 (die, parse_repo_url, detect_default_branch)
# --- cmd_clone ---     wd clone の実装
# --- usage ---         ヘルプ表示
# --- main ---          引数パース、サブコマンドディスパッチ
```

### 5.2 共通関数

| 関数                    | 説明                                           |
| ----------------------- | ---------------------------------------------- |
| `die <message>`         | エラーメッセージを stderr に出力して exit 1    |
| `parse_repo_url <url>`  | SSH URL を `host/owner/repo` 形式に正規化      |
| `detect_default_branch` | origin/HEAD → main → master の順でブランチ検出 |

## 6. 制約と互換性

| 項目       | 方針                                                                 |
| ---------- | -------------------------------------------------------------------- |
| シェル     | POSIX sh 互換 (`#!/bin/sh`)                                          |
| 禁止機能   | `local`, 配列, `[[ ]]`, `=~`, `pipefail`, プロセス置換等の bash 拡張 |
| OS         | Linux, macOS                                                         |
| Clone URL  | SSH (`git@host:owner/repo`) のみ                                     |
| 外部ツール | なし。git と POSIX sh のみ                                           |
