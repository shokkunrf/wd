# app/CLAUDE.md

wdの実行ファイルのcrate。cargoはこのディレクトリで実行する。`rust-toolchain.toml`はカレントディレクトリから探されるためである。

## コマンド

```sh
cargo fmt --check                                                  # 整形
cargo clippy --locked --all-targets -- -D warnings                 # lint
cargo test --locked                                                # テスト
cargo test --locked --target x86_64-unknown-linux-musl             # muslでのテスト
cargo deny --locked check -D warnings                              # 依存の許可リスト、advisory、ライセンス、取得元
cargo build --locked --release --target x86_64-unknown-linux-musl  # リリースと同じ静的リンクのビルド
```

CIは`RUSTFLAGS=-D warnings`で走る。`wd --version`が出すバージョンは、`Cargo.toml`の`version`である。静的リンクになったかは、`readelf -lW target/x86_64-unknown-linux-musl/release/wd`にINTERPが無く、`readelf -dW`にNEEDEDが無いことで確かめる。

## バージョンを揃える場所

上げるときは、同じ行の場所をすべて揃える。

| もの | 場所 |
| --- | --- |
| Rust | `rust-toolchain.toml`、`../.devcontainer/devcontainer.json`、shokkunrf/autopilot-sandboxの`workspaces/autopilot/.devcontainer/Dockerfile` |
| cargo-deny | `../.devcontainer/Dockerfile`、`../.github/workflows/ci.yml`、shokkunrf/autopilot-sandboxの`workspaces/autopilot/.devcontainer/Dockerfile` |
| cargo-about | `../.github/workflows/release.yml` |
| musl（Rustに同梱） | `../.github/workflows/ci.yml`の`static`の`MUSL_VERSION`と、`../.github/workflows/release.yml`の`MUSL_VERSION`と`MUSL_SHA256`。Rustを上げて同梱のmuslのバージョンが変わると、`static`が止まる |

## 実装規約

- エラーは自前のenumと手で書いた`Display`で表す。thiserrorとanyhowは使わない
- 標準出力のパイプの読み取り側が先に閉じたとき（`BrokenPipe`）は、エラーを出さずに0で終わる。`wd … | head`のような使い方と、`pipefail`のscriptを失敗させないため
- unwrap、expect、`panic!`、`todo!`、`unreachable!`、`let _ =`でResultを捨てることを禁じる。テストのコードでは許す。`print!`、`println!`、`eprint!`、`eprintln!`は書き込みに失敗するとpanicし、終了コードが0と1から外れるので、clippyの`print_stdout`と`print_stderr`で禁じ、出力は書き込みのエラーを扱う形で行う
- 警告をエラーにするのはCIの指定だけで、Cargo.tomlには書かない。clippyは既定のlintグループに個別のlintを足し、pedanticは入れない
- 検証済みの値は中身をprivateにした型にし、値を作る関数は検証の関数だけにする。コンテナ由来の文字列は受け取った時点で未検証の型に包み、中身を取り出すメソッドを検証と表示用のエスケープだけにする。未検証の型はジェネリクスにしない
- crateは1つ。コマンドはサブコマンドごとのモジュールに分け、ホストとコンテナのどちらで動いてよいかは各コマンドの定義に持たせ、dispatchで検査する。発行サービスは秘密鍵を扱うので`issuer`モジュールに分け、コマンドのモジュールとは互いに参照しない。共通の処理は`common`のような1つのモジュールにまとめず、`error`や`output`のように機能の名前のモジュールに置く
- 外部コマンドの呼び出しとファイルシステムへの操作はそれぞれ1か所のラッパーに集め、それ以外での直接の呼び出しを`disallowed-methods`で禁じる。開くことに限らず、`metadata`、`read_dir`、`remove_dir_all`、`canonicalize`のようにsymlinkを辿る操作も対象にする
- 発行サービスの秘密鍵を使う処理は、処理の時間が鍵に依らない実装で行い、タイミング攻撃について未修正のadvisoryがある実装は使わない。コンテナは要求を繰り返して応答の時間を測れるからである。暗号はringを使う。rsaはタイミング攻撃のadvisoryが未修正、aws-lc-rsは起動の最初に走るCの初期化を持つので使わない。JWTは自前で組む。HTTPSはureq v3とrustlsで、暗号の実装はring。ルート証明書はwebpki-rootsを埋め込み、その更新をリリースのきっかけにする
- 他のcrateはlexopt、serde、serde_json、libc、base64。libcは`O_NOFOLLOW`のような定数だけに使う。時刻の書式は自前で書き、ログのcrateは使わない。YAMLはsaphyr-parserのeventを読み、構造体へ手で移す。serde-saphyrは使わない。`deserialize`のfeatureが入れるencoding_rsが、x86_64でインラインアセンブリ（`asm!`）をコンパイルするからである。saphyr-parserの依存のthiserrorは入るが、wdのエラーには使わない
- Cやアセンブリを含む依存はringだけ。build scriptを持つcrateは個別に許可する。ctor、linkme、inventoryを禁じる
- 単体テストは同じファイルの`mod tests`。結合テストは`tests/`で実際のgitを使う。dockerが要るテストは`#[ignore]`にしてCIの別のジョブで走らせる。dev-dependenciesも許可リストの対象
- muslのビルドはmusl-toolsと`--target x86_64-unknown-linux-musl`で行う
- 自前のマクロ、async、実装が1つの自前のtraitとジェネリクス、参照を持つ構造体を禁じる
- レビューを行ったかは自動で検査せず、文で定める

## CIで検査する規約

| 規約 | 場所 |
| --- | --- |
| unsafeの禁止 | `Cargo.toml`の`[lints.rust]`の`unsafe_code = "forbid"` |
| unwrapなどとprint系の禁止 | `Cargo.toml`の`[lints.clippy]`。`dbg!`と`.ok()`でResultを捨てることも禁じる。テストのunwrap、expect、`panic!`は`clippy.toml`の`allow-*-in-tests`で許す。それ以外をテストで使うときと、結合テストの`#[test]`の外の補助関数では、その場所に`allow`を付ける |
| 外部コマンドとファイルシステムのラッパー | `clippy.toml`の`disallowed-methods`。ラッパーの関数にだけ`#[allow(clippy::disallowed_methods)]`を付ける。結合テストは実行ファイルと実際のgitを子プロセスとして実行するので、ファイル全体で許す |
| 依存の許可リスト、build script、禁じるcrate、advisory、ライセンス | `deny.toml`。crateの追加と更新は`bans.allow`のバージョンの変更を伴い、人が承認する |
| コンパイラのバージョン | `rust-toolchain.toml` |
| 静的リンク | `../.github/workflows/ci.yml`の`static`と`../.github/workflows/release.yml`の、readelfの検査 |

CIで検査しないものは、レビューで見る。

- `_ = expr;`でResultを捨てること。捕まえるlintが無い
- コマンドと`issuer`が互いに参照しないこと
- 各コマンドが動いてよい場所の検査
- 検証済みの型
- 自前のマクロ、async、参照を持つ構造体の禁止
- `deny.toml`の`licenses.allow`と`about.toml`の`accepted`が揃っていること。ずれると、リリースで止まる

標準エラーへの書き込みの失敗は報告する手段が無いので、`output::report`だけが文に`#[allow(clippy::let_underscore_must_use)]`を付けて捨てる。

## ファイル構成

- `src/main.rs` — 引数の解析とコマンドのdispatch
- `src/error.rs` — エラーの型
- `src/output.rs` — 標準出力と標準エラーへの書き込み
- `src/fs.rs` — ファイルを読むラッパー
- `src/yaml.rs` — YAMLの共通の読み取り。1文書を、所有する木に読む
- `src/workspace.rs` — workspace.yamlのキー表の検査
- `src/validate.rs` — `validate`サブコマンド
- `tests/` — 実行ファイルを子プロセスとして実行する結合テスト
- `about.toml`、`about.hbs` — リリースに同梱する依存の著作権表示の設定とテンプレート

サブコマンドのモジュールと`issuer`は、それを実装するtaskが`src/`に足す。
