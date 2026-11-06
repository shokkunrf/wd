# app/CLAUDE.md

wdの実行ファイルのcrate。cargoはこのディレクトリで実行する。`rust-toolchain.toml`はカレントディレクトリから探されるためである。

## コマンド

```sh
cargo fmt --check                                       # 整形
cargo clippy --locked --all-targets -- -D warnings      # lint
cargo test --locked                                     # テスト
cargo test --locked --target x86_64-unknown-linux-musl  # muslでのテスト
cargo deny --locked check -D warnings                   # 依存の許可リスト、勧告、ライセンス、取得元
cargo build --locked --release --target x86_64-unknown-linux-musl  # リリースと同じ静的リンクのビルド
```

CIは`RUSTFLAGS=-D warnings`で走る。静的リンクになったかは、`readelf -lW target/x86_64-unknown-linux-musl/release/wd`にINTERPが無く、`readelf -dW`にNEEDEDが無いことで確かめる。

## 実装規約

- エラーは自前のenumと手で書いた`Display`で表す。thiserrorとanyhowは使わない
- unwrap、expect、`panic!`、`todo!`、`unreachable!`、`let _ =`でResultを捨てることを禁じる。テストのコードでは許す。`print!`、`println!`、`eprint!`、`eprintln!`は書き込みに失敗するとpanicし、終了コードが0と1から外れるので、clippyの`print_stdout`と`print_stderr`で禁じ、出力は書き込みのエラーを扱う形で行う
- 警告をエラーにするのはCIの指定だけで、Cargo.tomlには書かない。clippyは既定の群に名指しのlintを足し、pedanticは入れない
- 検証済みの値は中身をprivateにした型にし、作る口は検証の関数だけにする。コンテナ由来の文字列は受け取った時点で未検証の型に包み、取り出す口を検証と表示用の無害化だけにする。未検証の型はジェネリクスにしない
- crateは1つ。役割ごとのモジュール、ホスト専用、どこでも、internal、発行サービス、と共通のモジュールに分け、役割のモジュール同士は参照しない
- 外部コマンドの呼び出しとファイルシステムへの操作はそれぞれ1か所のラッパに集め、それ以外での直接の呼び出しを`disallowed-methods`で禁じる。開くことに限らず、`metadata`、`read_dir`、`remove_dir_all`、`canonicalize`のようにsymlinkを辿る操作も対象にする
- 発行サービスの秘密鍵を使う処理は、処理の時間が鍵に依らない実装で行い、時間の漏れについて未修正の勧告がある実装は使わない。コンテナは要求を繰り返して応答の時間を測れるからである。暗号はringを使う。rsaは時間の勧告が未修正、aws-lc-rsは起動の最初に走るCの初期化を持つので使わない。JWTは自前で組む。HTTPSはureq v3とrustlsで、暗号の実装はring。ルート証明書はwebpki-rootsを埋め込み、その更新をリリースのきっかけにする
- 他のcrateはlexopt、serde、serde_json、libc、base64。libcは`O_NOFOLLOW`のような定数だけに使う。時刻の書式は自前で書き、ログのcrateは使わない。YAMLはserde-saphyrでserdeの構造体に移す。serde-saphyrとその依存を許可リストに足すのはwd#18で、wd#18が依存の中身を確かめてから足す
- Cやアセンブリを含む依存はringだけ。build scriptを持つcrateは名指しで許す。ctor、linkme、inventoryを禁じる
- 単体テストは同じファイルの`mod tests`。結合テストは`tests/`で本物のgitを使う。dockerが要る試験は`#[ignore]`にしてCIの別のジョブで走らせる。dev-dependenciesも許可リストの対象
- muslのビルドはmusl-toolsと`--target x86_64-unknown-linux-musl`で行う
- 自前のマクロ、async、実装が1つの自前のtraitとジェネリクス、参照を持つ構造体を禁じる
- レビューを行ったかは機械で確かめず、文で定める

## 規約を機械で確かめる場所

| 規約 | 場所 |
|---|---|
| unsafeの禁止 | `Cargo.toml`の`[lints.rust]`の`unsafe_code = "forbid"` |
| unwrapなどとprint系の禁止 | `Cargo.toml`の`[lints.clippy]`。`dbg!`と`.ok()`でResultを捨てることも禁じる。テストのunwrap、expect、`panic!`は`clippy.toml`の`allow-*-in-tests`で許す。それ以外をテストで使うときと、結合テストの`#[test]`の外の補助では、その場所に`allow`を付ける |
| 外部コマンドとファイルシステムのラッパ | `clippy.toml`の`disallowed-methods`。ラッパの関数にだけ`#[allow(clippy::disallowed_methods)]`を付ける。結合テストは実行ファイルと本物のgitを外から呼ぶので、ファイル全体で許す |
| 依存の許可リスト、build script、禁じるcrate、勧告、ライセンス | `deny.toml`。crateの追加と更新は`bans.allow`の版の変更を伴い、人が承認する |
| ライセンスの許可 | `deny.toml`の`licenses.allow`と`about.toml`の`accepted`。両方を揃えて変える |
| コンパイラのバージョン | `rust-toolchain.toml` |
| 静的リンク | `.github/scripts/release-assets.sh`のreadelfの検査。CIとリリースが同じものを走らせる |

機械で確かめないもの。`_ = expr;`でResultを捨てることは捕まえるlintが無いので、レビューで見る。標準エラーへの書き込みの失敗だけは伝える先が無いので、`common::output::report`が文に`#[allow(clippy::let_underscore_must_use)]`を付けて捨てる。役割のモジュール同士を参照しないこと、検証済みの型、自前のマクロとasyncと参照を持つ構造体の禁止も、レビューで見る。

## ファイル構成

- `src/main.rs` — 引数の振り分け
- `src/common/` — 役割に共通のもの。エラー、出力、バージョン
- `tests/` — 実行ファイルを外から呼ぶ結合テスト
- `about.toml`、`about.hbs` — リリースに同梱する依存の著作権表示の設定と雛形

役割ごとのモジュール、`host`、`anywhere`、`internal`、`issuer`は、それを実装するtaskが`src/`に足す。
