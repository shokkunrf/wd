#!/bin/sh
# test_integration.sh — integration tests for wd
set -eu

cd "$(dirname "$0")/.."
. tests/test_helpers.sh

WD_TESTING=1
export WD_TESTING
. src/wd.sh

# ============================================================
describe "cmd_clone"
# ============================================================

setup_test_env

_source=$(create_local_bare_repo "$_test_tmpdir/source")
_orig_parse_repo_url=$(type parse_repo_url | tail -n +2)

# --- non-empty repo ---

it "clones repository with bare + worktree structure"
# shellcheck disable=SC2317
parse_repo_url() { echo "github.com/test/repo"; } # monkey-patch
_result=$(cmd_clone "$_source" 2>&3)
assert_contains "$_result" "github.com/test/repo"

it "creates .bare directory"
_proj="$WD_ROOT/github.com/test/repo"
if [ -d "$_proj/.bare" ]; then _pass; else _fail ".bare not found"; fi

it "creates .git pointer file"
if [ -f "$_proj/.git" ]; then _pass; else _fail ".git not found"; fi

it "creates default worktree"
if [ -d "$_proj/main" ]; then _pass; else _fail "main worktree not found"; fi

it "creates .devcontainer symlink"
if [ -L "$_proj/.devcontainer" ]; then _pass; else _fail ".devcontainer symlink not found"; fi

it "clones with -b to specify branch"
# shellcheck disable=SC2317
parse_repo_url() { echo "github.com/test/override"; } # monkey-patch
_result=$(cmd_clone "$_source" -b main 2>&3)
_proj="$WD_ROOT/github.com/test/override"
if [ -d "$_proj/main" ]; then _pass; else _fail "main worktree not found"; fi

it "rejects nonexistent branch with -b"
# shellcheck disable=SC2317
parse_repo_url() { echo "github.com/test/noexist"; } # monkey-patch
assert_exit_code 1 cmd_clone "$_source" -b nonexistent

it "rejects duplicate clone"
# shellcheck disable=SC2317
parse_repo_url() { echo "github.com/test/repo"; } # monkey-patch
assert_exit_code 1 cmd_clone "$_source"

# --- empty repo ---

_empty_source=$(create_local_empty_repo "$_test_tmpdir/empty")

it "clones empty repo with default branch"
# shellcheck disable=SC2317
parse_repo_url() { echo "github.com/test/empty1"; } # monkey-patch
_result=$(cmd_clone "$_empty_source" 2>&3)
_proj="$WD_ROOT/github.com/test/empty1"
if [ -d "$_proj/main" ]; then _pass; else _fail "main worktree not found"; fi

it "clones empty repo with -b specifying default branch"
# shellcheck disable=SC2317
parse_repo_url() { echo "github.com/test/empty2"; } # monkey-patch
_result=$(cmd_clone "$_empty_source" -b main 2>&3)
_proj="$WD_ROOT/github.com/test/empty2"
if [ -d "$_proj/main" ]; then _pass; else _fail "main worktree not found"; fi

it "rejects empty repo with -b specifying nonexistent branch"
# shellcheck disable=SC2317
parse_repo_url() { echo "github.com/test/empty3"; } # monkey-patch
assert_exit_code 1 cmd_clone "$_empty_source" -b feature

# --- argument errors ---

it "fails with no arguments"
assert_exit_code 1 cmd_clone

it "fails with -b but no value"
assert_exit_code 1 cmd_clone "$_source" -b

it "fails with unknown option"
assert_exit_code 1 cmd_clone "$_source" --unknown

it "fails with too many arguments"
assert_exit_code 1 cmd_clone "$_source" extra

eval "$_orig_parse_repo_url"
unset _orig_parse_repo_url

teardown_test_env

# ============================================================
describe "cmd_add"
# ============================================================

setup_test_env

_source=$(create_local_bare_repo "$_test_tmpdir/source_add")
_orig_parse_repo_url=$(type parse_repo_url | tail -n +2)
# shellcheck disable=SC2317
parse_repo_url() { echo "github.com/test/add-test"; } # monkey-patch
_result=$(cmd_clone "$_source" 2>&3)
_proj="$WD_ROOT/github.com/test/add-test"

# Create an extra branch in the source repo for testing
git -C "$_source" checkout -b feature-1 2>&3 >&3
echo "feature" >"$_source/feature.txt"
git -C "$_source" add -A 2>&3 >&3
git -C "$_source" commit -m "feature commit" 2>&3 >&3
git -C "$_proj" fetch origin 2>&3 >&3

# Create a PR-like ref in the source repo
git -C "$_source" update-ref refs/pull/12/head HEAD

cd "$_proj/main"

it "adds worktree with wt- prefix"
_result=$(cmd_add feature-1 2>&3)
if [ -d "$_proj/wt-feature-1" ]; then _pass; else _fail "worktree not found"; fi

it "rejects duplicate worktree"
assert_exit_code 1 cmd_add feature-1

it "creates new branch with -b and wt- prefix"
_result=$(cmd_add -b newbranch 2>&3)
if [ -d "$_proj/wt-newbranch" ]; then _pass; else _fail "worktree not found"; fi

it "adds PR worktree with pr- prefix"
_result=$(cmd_add --pr 12 2>&3)
if [ -d "$_proj/pr-12" ]; then _pass; else _fail "pr-12 directory not found"; fi

it "outputs correct path for PR worktree"
assert_contains "$_result" "pr-12"

it "rejects duplicate PR worktree"
assert_exit_code 1 cmd_add --pr 12

# --- argument errors ---

it "--pr requires an argument"
assert_exit_code 1 cmd_add --pr

it "fails with no arguments"
assert_exit_code 1 cmd_add

it "fails with unknown option"
assert_exit_code 1 cmd_add --unknown

it "fails with too many arguments"
assert_exit_code 1 cmd_add branch1 branch2

it "fails outside project"
cd "$_test_tmpdir"
assert_exit_code 1 cmd_add somebranch

eval "$_orig_parse_repo_url"
unset _orig_parse_repo_url

teardown_test_env

# ============================================================
describe "cmd_remove"
# ============================================================

setup_test_env

_source=$(create_local_bare_repo "$_test_tmpdir/source_remove")
_orig_parse_repo_url=$(type parse_repo_url | tail -n +2)
# shellcheck disable=SC2317
parse_repo_url() { echo "github.com/test/remove-test"; } # monkey-patch
_result=$(cmd_clone "$_source" 2>&3)
_proj="$WD_ROOT/github.com/test/remove-test"

cd "$_proj/main"

it "removes a single worktree"
_result=$(cmd_add -b feature-rm 2>&3)
cmd_remove wt-feature-rm 2>&3 >&3
if [ ! -d "$_proj/wt-feature-rm" ]; then _pass; else _fail "worktree still exists"; fi

it "removes multiple worktrees"
_result=$(cmd_add -b multi-a 2>&3)
_result=$(cmd_add -b multi-b 2>&3)
cmd_remove wt-multi-a wt-multi-b 2>&3 >&3
if [ ! -d "$_proj/wt-multi-a" ] && [ ! -d "$_proj/wt-multi-b" ]; then _pass; else _fail "worktrees still exist"; fi

it "rejects removing default worktree"
assert_exit_code 1 cmd_remove main

it "preserves branch without -b flag"
_result=$(cmd_add -b branch-keep 2>&3)
cmd_remove wt-branch-keep 2>&3 >&3
_has_branch=0
git -C "$_proj" branch --list "branch-keep" | grep -q "branch-keep" && _has_branch=1
if [ "$_has_branch" = 1 ]; then _pass; else _fail "branch should be preserved"; fi

it "deletes branch with -b flag"
_result=$(cmd_add -b branch-del 2>&3)
cmd_remove -b wt-branch-del 2>&3 >&3
_has_branch=0
git -C "$_proj" branch --list "branch-del" | grep -q "branch-del" && _has_branch=1
if [ "$_has_branch" = 0 ]; then _pass; else _fail "branch should be deleted"; fi

it "deletes branch with --branch flag"
_result=$(cmd_add -b branch-del2 2>&3)
cmd_remove --branch wt-branch-del2 2>&3 >&3
_has_branch=0
git -C "$_proj" branch --list "branch-del2" | grep -q "branch-del2" && _has_branch=1
if [ "$_has_branch" = 0 ]; then _pass; else _fail "branch should be deleted"; fi

it "removes all non-default worktrees with -a"
_result=$(cmd_add -b all-a 2>&3)
_result=$(cmd_add -b all-b 2>&3)
cmd_remove -a 2>&3 >&3
if [ -d "$_proj/main" ] && [ ! -d "$_proj/wt-all-a" ] && [ ! -d "$_proj/wt-all-b" ]; then _pass; else _fail "non-default should be removed"; fi

it "-a with no worktrees is silent"
_output=$(cmd_remove -a 2>&1)
if [ -z "$_output" ]; then _pass; else _fail "expected no output, got: $_output"; fi

it "fails for nonexistent worktree"
assert_exit_code 1 cmd_remove wt-nonexistent

# --- argument errors ---

it "fails with no arguments"
assert_exit_code 1 cmd_remove

it "fails with unknown option"
assert_exit_code 1 cmd_remove --unknown

it "fails outside project"
cd "$_test_tmpdir"
assert_exit_code 1 cmd_remove wt-somebranch

eval "$_orig_parse_repo_url"
unset _orig_parse_repo_url

teardown_test_env

# ============================================================

test_summary
