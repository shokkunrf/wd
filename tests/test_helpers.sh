#!/bin/sh
# test_helpers.sh — POSIX sh test framework for wd
set -eu

# --- test framework ---

_test_pass=0
_test_fail=0
_test_name=""
_test_verbose=false
for _arg; do
  case "$_arg" in -v | --verbose) _test_verbose=true ;; esac
done
unset _arg

# fd 3: verbose output (stderr when -v, /dev/null otherwise)
if $_test_verbose; then
  exec 3>&2
else
  exec 3>/dev/null
fi

describe() {
  echo ""
  echo "=== $1 ==="
}

it() {
  _test_name="$1"
}

_pass() {
  _test_pass=$((_test_pass + 1))
  echo "  PASS: $_test_name"
}

_fail() {
  _test_fail=$((_test_fail + 1))
  echo "  FAIL: $_test_name — $1" >&2
}

assert_eq() {
  _expected="$1"
  _actual="$2"
  if [ "$_expected" = "$_actual" ]; then
    _pass
  else
    _fail "expected '$_expected', got '$_actual'"
  fi
  unset _expected _actual
}

assert_contains() {
  _haystack="$1"
  _needle="$2"
  case "$_haystack" in
  *"$_needle"*)
    _pass
    ;;
  *)
    _fail "'$_haystack' does not contain '$_needle'"
    ;;
  esac
  unset _haystack _needle
}

assert_exit_code() {
  _expected_code="$1"
  shift
  _actual_code=0
  ("$@") >/dev/null 2>&1 || _actual_code=$?
  if [ "$_expected_code" = "$_actual_code" ]; then
    _pass
  else
    _fail "expected exit code $_expected_code, got $_actual_code"
  fi
  unset _expected_code _actual_code
}

test_summary() {
  echo ""
  echo "--- Results: $_test_pass passed, $_test_fail failed ---"
  return "$_test_fail"
}

# --- fixture helpers ---

setup_test_env() {
  _test_tmpdir=$(mktemp -d)
  WD_ROOT="$_test_tmpdir/root"
  mkdir -p "$WD_ROOT"
  export WD_ROOT
}

teardown_test_env() {
  rm -rf "$_test_tmpdir"
  unset _test_tmpdir WD_ROOT
}

# create_local_empty_repo <dir>
#   Creates an empty git repo (no commits), returns the path.
create_local_empty_repo() {
  _repo_dir="$1"
  mkdir -p "$_repo_dir"
  git -C "$_repo_dir" init --initial-branch=main 2>&3 >&3
  echo "$_repo_dir"
  unset _repo_dir
}

# create_local_bare_repo <dir> [<branch>]
#   Creates a normal git repo with an initial commit and .devcontainer,
#   then returns the path (usable as a file:// clone source).
create_local_bare_repo() {
  _repo_dir="$1"
  _repo_branch="${2:-main}"
  mkdir -p "$_repo_dir"
  git -C "$_repo_dir" init --initial-branch="$_repo_branch" 2>&3 >&3
  mkdir -p "$_repo_dir/.devcontainer"
  echo '{}' >"$_repo_dir/.devcontainer/devcontainer.json"
  git -C "$_repo_dir" add -A 2>&3 >&3
  git -C "$_repo_dir" commit -m "initial" 2>&3 >&3
  echo "$_repo_dir"
  unset _repo_dir _repo_branch
}
