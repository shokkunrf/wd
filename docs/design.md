# `wd` 技術設計書

**wd** — git worktree manager with devcontainer support

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
            ├── <default-branch>/          # デフォルトワークツリー (clone時に作成)
            ├── wt-<branch>/               # wd add で追加したワークツリー
            └── pr-<number>/               # wd add --pr で追加した PR ワークツリー
```

### 2.1 プロジェクトの識別

- **プロジェクトルート**: `.bare/` ディレクトリを含むディレクトリ
- **検出**: カレントディレクトリから親方向へ `.bare/` を探索

### 2.2 ワークツリーの命名規則

| 種別             | ディレクトリ名   | 作成元                |
| ---------------- | ---------------- | --------------------- |
| デフォルト       | `<branch>`       | `wd clone`            |
| ブランチ作業用   | `wt-<branch>`    | `wd add`              |
| PR レビュー用    | `pr-<number>`    | `wd add --pr`         |

デフォルトワークツリーは `.devcontainer` symlink の参照先として保護され、`wd remove` で削除できない。

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
   a. -b 指定あり → そのブランチ (リモートに存在しなければエラー)
   b. git symbolic-ref HEAD でベアリポジトリの HEAD から取得
   c. いずれも見つからない → エラー (-b を案内)
9. リモートにブランチが存在 → git worktree add <branch> <branch>
   空リポジトリ → git worktree add --orphan -b <branch> <branch>
10. <branch>/.devcontainer が存在すれば symlink を作成
11. プロジェクトディレクトリのパスを stdout に出力
```

### 3.2 `wd list` — プロジェクト一覧

**書式**: `wd list [--full-path] [--worktrees]`

**処理フロー**:

```
1. $WD_ROOT 配下から .bare ディレクトリを検索 (glob: */*/*)
2. 各 .bare の親ディレクトリがプロジェクトルート
3. $WD_ROOT/ プレフィックスを除去して host/owner/repo 形式で出力
4. --full-path: 絶対パスで出力
5. --worktrees: 各プロジェクトのワークツリーも一覧に含める
```

**出力例**:

```
github.com/owner1/repo1
github.com/owner2/repo2
```

`--worktrees` 使用時:

```
github.com/owner1/repo1/main
github.com/owner1/repo1/wt-feature-x
github.com/owner2/repo2/main
```

### 3.3 `wd add` — ワークツリー追加

**書式**: `wd add <branch> [-b]` / `wd add --pr <number>`

**処理フロー**:

```
1. カレントディレクトリから親方向へ .bare/ を探索してプロジェクトルートを特定
2. プロジェクトルートが見つからない → エラー終了
3. --pr 指定:
   a. pr-<number> ディレクトリが既存なら → エラー終了
   b. git fetch origin pull/<number>/head:pr-<number>
   c. git worktree add pr-<number> pr-<number>
4. 通常:
   a. wt-<branch> ディレクトリが既存なら → エラー終了
   b. -b 指定あり → git worktree add wt-<branch> -b <branch> (新規ブランチ作成)
      -b 指定なし → git worktree add wt-<branch> <branch> (既存ブランチをチェックアウト)
5. 作成されたワークツリーのパスを stdout に出力
```

### 3.4 `wd remove` — ワークツリー削除

**書式**: `wd remove <name>... [-b|--branch]` / `wd remove -a [-b|--branch]`

**処理フロー**:

```
1. カレントディレクトリから親方向へ .bare/ を探索してプロジェクトルートを特定
2. プロジェクトルートが見つからない → エラー終了
3. デフォルトワークツリーを特定 (.devcontainer symlink の参照先、または wt-/pr- 以外のワークツリー)
4. -a 指定 → デフォルト以外の全ワークツリーを対象
5. デフォルトワークツリーの削除は拒否
6. 指定されたワークツリーが存在しない → エラー終了
7. git worktree remove でワークツリーを削除
8. -b/--branch 指定時は対応するブランチも git branch -D で削除
```

### 3.5 `wd repair` — ワークツリーパス修復

**書式**: `wd repair`

**背景**: `git worktree add`は`.git`と`gitdir`を絶対パスで設定するため、ホストとdevcontainerでパスが食い違い、gitが機能しなくなる。Git v2.48+の`--relative-paths`で解決できるが、Debian TrixieのGit v2.47.3では使えないため、`wd`は`write_relative_paths`で相対パスに書き換えている。`git worktree repair`を実行すると絶対パスに戻されるが、`wd repair`で再修復する。

**処理フロー**:

```
1. カレントディレクトリから親方向へ .bare/を探索してプロジェクトルートを特定
2. プロジェクトルートが見つからない → エラー終了
3. .bare/worktrees/ディレクトリが存在しない → 何もせず正常終了
4. .bare/worktrees/配下の各エントリをスキャン:
   a. 対応するワークツリーディレクトリが存在しない → スキップ (手動削除済み)
   b. 存在する → write_relative_pathsでパスを再書き込み
5. 修復したワークツリー名をstdoutに出力
```

**設計判断**:

- `git worktree list`ではなく `.bare/worktrees/*/`を直接スキャンする → gitがパスを解決できない状態でも動作
- 引数なし。カレントプロジェクトの全ワークツリーを一括修復

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

# --- utils ---         共通関数 (die, parse_repo_url, find_project_root, list_worktrees, get_default_worktree)
# --- cmd_clone ---     wd clone の実装
# --- cmd_list ---      wd list の実装
# --- cmd_add ---       wd add の実装
# --- cmd_remove ---    wd remove の実装 (_remove_one ヘルパー含む)
# --- cmd_repair ---    wd repair の実装
# --- usage ---         ヘルプ表示
# --- main ---          引数パース、サブコマンドディスパッチ
```

### 5.2 共通関数

| 関数                                  | 説明                                                                  |
| ------------------------------------- | --------------------------------------------------------------------- |
| `die <message>`                       | エラーメッセージを stderr に出力して exit 1                           |
| `parse_repo_url <url>`                | SSH URL を `host/owner/repo` 形式に正規化                             |
| `find_project_root`                   | カレントディレクトリから親方向へ `.bare/` を探索                      |
| `list_worktrees <project_root>`       | プロジェクト内のワークツリー名一覧を出力 (bare エントリ除外)          |
| `get_default_worktree <project_root>` | デフォルトワークツリー名を返す (.devcontainer 参照先 or wt-/pr- 以外) |
| `write_relative_paths <root> <name>`  | ワークツリーの `.git` と `gitdir` を相対パスで再書き込み              |

## 6. 制約と互換性

| 項目       | 方針                                                                 |
| ---------- | -------------------------------------------------------------------- |
| シェル     | POSIX sh 互換 (`#!/bin/sh`)                                          |
| 禁止機能   | `local`, 配列, `[[ ]]`, `=~`, `pipefail`, プロセス置換等の bash 拡張 |
| Git        | >= 2.17.0（`git worktree add` の基本機能に依存）                     |
| OS         | Linux, macOS                                                         |
| Clone URL  | SSH (`git@host:owner/repo`) のみ                                     |
| 外部ツール | なし。git と POSIX sh のみ                                           |
