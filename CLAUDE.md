# CLAUDE.md

`wd`プロジェクトの開発ガイドライン。

## プロジェクト概要

wdはworkspaceとinstanceを扱うCLIで、Rustで書く。配るのは1つの実行ファイルで、ホストのコマンド、コンテナのコマンド、発行サービスをサブコマンドで切り替える。仕様の正本はshokkunrf/autopilot-sandboxの`docs/design/workspace-spec.md`と`docs/design/instance-spec.md`で、決定はそこの`docs/adr/`にある。

## コマンド

```sh
prettier --write "**/*.md"  # Markdownの整形
```

## ファイル構成

- `service/` — shellの発行サービス。wd#24で置き換えるまで使う
- `install.sh` — Releaseからwdをインストールするスクリプト
- `README.md`、`docs/` — shellのwdの説明。wd#30で書き直す
- `.devcontainer/` — 開発コンテナ
