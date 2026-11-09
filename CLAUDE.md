# CLAUDE.md

`wd`プロジェクトの開発ガイドライン。

## プロジェクト概要

wdはworkspaceとinstanceを扱うCLIで、Rustで書く。配るのは1つの実行ファイルで、ホストのコマンド、コンテナのコマンド、発行サービスをサブコマンドで切り替える。仕様の正本はshokkunrf/autopilot-sandboxの`docs/design/workspace-spec.md`と`docs/design/instance-spec.md`で、決定はそこの`docs/adr/`にある。実行ファイルのcrateは`app/`にあり、Rustの規約とコマンドは`app/CLAUDE.md`に書く。

## commitの規約

- commitの前に、サブエージェントによるレビューを必ず行う
- commitの前に、整形とlintとテストを走らせる
- commitメッセージは1行で、何のために何をしたかを書く。トレーラーは書かない。2行以上が要るcommitは、レビューするには大きすぎるので分ける。squashマージのcommitは、本文に各commitのメッセージや、GitHubが足すトレーラーが並んでよい

## 言語

- エラーなどの出力と、コード（Rust、shell、workflowのstepの名前、TOMLやDockerfileのコメント）は英語で書く。コードの中で日本語を使わない
- commitメッセージとPRの題は英語で書く。PRの題は、squashマージのcommitの題になるため
- PRの本文、Issue、CLAUDE.mdは日本語で書く

## workflowの書き方

botが書いたworkflowはマージの前のブランチでも動くので、影響の及ぶ範囲を狭める。

- workflowの先頭に`permissions: {}`を書き、jobごとに最小の`permissions`を書く
- `uses:`のactionはタグで書き、commitは`.github/workflows/actions.lock`で固定する。使うactionはshokkunrf/wdのGitHub Actionsの許可リストに入れ、追加はcrateと同じく人が承認する
- workflowはリポのsecretを使わない
- workflowを変えたら、`gh actions-lock`で`actions.lock`を作り直し、同じcommitに含める。`verify-lockfile.yml`が`gh actions-lock --verify`で照合する

## リリース

Releaseは人がGitHubで、`develop`のcommitから作る。botとセッションは、Releaseもタグも作らない。タグは`v<app/Cargo.tomlのバージョン>`にし、その前に`app/Cargo.toml`の`version`を上げるPRをマージしておく。

Releaseを公開すると`.github/workflows/release.yml`が走り、muslで静的リンクした実行ファイルを作って、`wd-<バージョン>-x86_64-unknown-linux-musl.tar.gz`と`SHA256SUMS`をそのReleaseにアップロードする。タグと`app/Cargo.toml`のバージョンが違えば止まる。

wd#25でホストをRustのwdに切り替えるまでは、pre-releaseとして出す。pre-releaseでないReleaseは`latest`になり、shellのwdの`wd update`と、`ghcr.io/shokkunrf/devcontainer-features/wd`のfeatureの既定は、`latest`のReleaseからwdをインストールするためである。

## コマンド

```sh
prettier --write "**/*.md"                                                      # Markdownの整形
gh extension install github/gh-actions-lock --pin v0.4.0                        # gh-actions-lockのインストール
gh actions-lock                                                                 # `actions.lock`の作り直し
shellcheck install.sh service/install service/wd-token-issuer                   # shellのlint。CIと同じ
shfmt -d -ln posix -i 2 -bn install.sh service/install service/wd-token-issuer  # shellの整形の検査。CIと同じ
```

CIのshellcheckとshfmtは、開発コンテナのDebian trixieのaptと同じバージョン（0.10.0と3.8.0）にする。shokkunrf/autopilot-sandboxの`workspaces/autopilot/.devcontainer/Dockerfile`も、同じくDebian trixieのaptでインストールする。

## ファイル構成

- `app/` — 実行ファイルのcrate
- `service/` — shellの発行サービス。wd#24で置き換えるまで使う
- `install.sh` — Releaseからwdをインストールするスクリプト
- `README.md`、`docs/` — shellのwdの説明。wd#30で書き直す
- `.devcontainer/` — 開発コンテナ。Rustのtoolchainは`rust` featureで、musl-tools、shellcheck、shfmt、cargo-denyはDockerfileでインストールする
- `.github/workflows/` — CIとリリースと、使うactionのcommitを固定する`actions.lock`
