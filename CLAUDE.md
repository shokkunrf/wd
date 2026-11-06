# CLAUDE.md

`wd`プロジェクトの開発ガイドライン。

## プロジェクト概要

wdはworkspaceとinstanceを扱うCLIで、Rustで書く。配る実行物は1つの実行ファイルで、役割は起動のときのサブコマンドだけで決まる。仕様の正本はshokkunrf/autopilot-sandboxの`docs/design/workspace-spec.md`と`docs/design/instance-spec.md`で、決定はそこの`docs/adr/`にある。実行ファイルのcrateは`app/`にあり、Rustの規約とコマンドは`app/CLAUDE.md`に書く。

## commitの規約

- commitの前に、サブエージェントによるレビューを必ず行う
- commitの前に、整形とlintとテストを走らせる
- commitメッセージは1行で、何のために何をしたかを書く。トレーラーは書かない

## workflowの書き方

botが書いたworkflowはマージの前のブランチでも動くので、影響の及ぶ範囲を狭める。

- jobごとに最小の`permissions`を書く
- `uses:`のactionはcommitのSHAで固定する。使うactionはshokkunrf/wdのGitHub Actionsの許可リストに入れ、追加はcrateと同じく人が承認する
- workflowはリポのsecretを使わない

## ファイル構成

- `app/` — 実行ファイルのcrate
- `service/` — shellの発行サービス。wd#24で置き換えるまで使う
- `install.sh` — Releaseからwdを入れるインストーラ
- `.devcontainer/` — 開発コンテナ。Rustの道具はautopilot-sandboxのworkspaceのコンテナと同じ
- `.github/workflows/` — CI
