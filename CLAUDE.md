# CLAUDE.md

`wd`プロジェクトの開発ガイドライン。

## プロジェクト概要

bare clone + git worktreeによるリポジトリ管理CLI。単一のPOSIX shスクリプト。

## 開発ルール

- **POSIX sh互換** — `#!/bin/sh`で動作すること。`local`, 配列, `[[ ]]`, `pipefail`等のbash拡張は禁止
- **単一ファイル** — `wd`ファイルに全コマンドを実装。ビルドステップなし
- **セクション順序** — utils → cmd_clone → usage → main
- **変数名** — グローバル変数汚染を避けるため`_`接頭辞を使用し、関数末尾で`unset`する

## コマンド

```sh
shfmt -d -ln posix -i 2 -bn $(find . -name '*.sh')  # shfmtによるフォーマットチェック
shellcheck -s sh $(find . -name '*.sh')  # ShellCheckによるlint
sh tests/test_unit.sh && sh tests/test_integration.sh  # テスト実行
sh src/wd.sh --version              # 動作確認
dash src/wd.sh --version            # POSIX互換確認
prettier --write "**/*.md"          # Markdownフォーマット
```

## ファイル構成

- `src/wd.sh` — 本体（インストール時に`wd`としてコピー）
- `docs/design.md` — 技術設計書
- `install.sh` — インストーラ
- `.github/workflows/` — CI/CDワークフロー
