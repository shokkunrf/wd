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

## リリース

`v<semver>`のタグをpushすると`.github/workflows/release.yml`が走る。タグから先頭の`v`を除いたものを`WD_VERSION`として実行ファイルに焼き込み、`.github/scripts/release-assets.sh`で検査してまとめ、`wd-<バージョン>-x86_64-unknown-linux-musl.tar.gz`、`SHA256SUMS`、バージョンを焼き込んだ`install.sh`をReleaseに上げる。バージョンに`-`があればpre-releaseにする。

wd#25でホストをRustのwdに切り替えるまでは、pre-releaseのタグだけを打つ。pre-releaseでないReleaseは`latest`になり、shellのwdの`wd update`と、`ghcr.io/shokkunrf/devcontainer-features/wd`のfeatureの既定が、それを入れるようになるためである。`install.sh`をReleaseに上げるのは、これらが`releases/latest/download/install.sh`と`releases/download/<タグ>/install.sh`から取るためである。

CIも`release-assets.sh`を同じ手順で走らせる。手元で走らせるには、musl-tools、cargo-about 0.9.3、dockerが要る。cargo-aboutとdockerは開発コンテナに無い。

```sh
WD_VERSION=0.0.0-ci .github/scripts/release-assets.sh <出力先>
```

## ファイル構成

- `app/` — 実行ファイルのcrate
- `service/` — shellの発行サービス。wd#24で置き換えるまで使う
- `install.sh` — Releaseのtar.gzからwdを入れるインストーラ。Releaseに上げるときにバージョンを焼き込む
- `.devcontainer/` — 開発コンテナ。Rustの道具はautopilot-sandboxのworkspaceのコンテナと同じ
- `.github/workflows/` — CIとリリース
- `.github/scripts/release-assets.sh` — リリースと同じ手順で実行ファイルを作って検査し、Releaseに上げるものをまとめる。CIとリリースが呼ぶ
