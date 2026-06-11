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
describe "cmd_list"
# ============================================================

setup_test_env

_source1=$(create_local_bare_repo "$_test_tmpdir/source_list1")
_source2=$(create_local_bare_repo "$_test_tmpdir/source_list2")
_orig_parse_repo_url=$(type parse_repo_url | tail -n +2)

# shellcheck disable=SC2317
parse_repo_url() { echo "github.com/owner1/repo1"; } # monkey-patch
_result=$(cmd_clone "$_source1" 2>&3)
_proj1="$WD_ROOT/github.com/owner1/repo1"

# shellcheck disable=SC2317
parse_repo_url() { echo "github.com/owner2/repo2"; } # monkey-patch
_result=$(cmd_clone "$_source2" 2>&3)

it "lists managed projects"
_result=$(cmd_list)
assert_contains "$_result" "github.com/owner1/repo1"
assert_contains "$_result" "github.com/owner2/repo2"

it "ignores directories without .bare"
mkdir -p "$WD_ROOT/github.com/owner3/repo3"
_result=$(cmd_list)
assert_not_contains "$_result" "owner3/repo3"

it "shows full path with --full-path"
_result=$(cmd_list --full-path)
assert_contains "$_result" "$WD_ROOT/github.com/owner1/repo1"

it "shows worktree paths with --worktrees"
cd "$_proj1/main"
_result=$(cmd_add -b feature-x 2>&3)
_result=$(cmd_list --worktrees)
assert_contains "$_result" "github.com/owner1/repo1/main"
assert_contains "$_result" "github.com/owner1/repo1/wt-feature-x"

it "returns empty for no projects"
teardown_test_env
setup_test_env
_result=$(cmd_list)
if [ -z "$_result" ]; then _pass; else _fail "expected empty output, got: $_result"; fi

# --- argument errors ---

it "fails with unknown option"
assert_exit_code 1 cmd_list --unknown

it "fails with too many arguments"
assert_exit_code 1 cmd_list repo1

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

it "new branch without base starts from the default branch"
_base_sha=$(git -C "$_proj" rev-parse main)
_new_sha=$(git -C "$_proj" rev-parse newbranch)
assert_eq "$_base_sha" "$_new_sha"

it "rejects duplicate worktree with -b"
assert_exit_code 1 cmd_add -b newbranch

it "creates new branch from a base with -b <branch> <base>"
_result=$(cmd_add -b from-base feature-1 2>&3)
if [ -d "$_proj/wt-from-base" ]; then _pass; else _fail "worktree not found"; fi

it "new branch points at the base commit"
_base_sha=$(git -C "$_proj" rev-parse feature-1)
_new_sha=$(git -C "$_proj" rev-parse from-base)
assert_eq "$_base_sha" "$_new_sha"

it "writes relative gitdir path for the -b worktree"
assert_eq "gitdir: ../.bare/worktrees/wt-from-base" "$(cat "$_proj/wt-from-base/.git")"

it "creates new branch from a remote-tracking base"
_result=$(cmd_add -b from-remote origin/feature-1 2>&3)
_base_sha=$(git -C "$_proj" rev-parse origin/feature-1)
_new_sha=$(git -C "$_proj" rev-parse from-remote)
assert_eq "$_base_sha" "$_new_sha"

it "creates new branch from a tag base"
git -C "$_source" tag v1.0 feature-1 2>&3 >&3
git -C "$_proj" fetch origin --tags 2>&3 >&3
_result=$(cmd_add -b from-tag v1.0 2>&3)
_base_sha=$(git -C "$_proj" rev-parse "v1.0^{commit}")
_new_sha=$(git -C "$_proj" rev-parse from-tag)
assert_eq "$_base_sha" "$_new_sha"

it "creates new branch from a commit-hash base"
_hash=$(git -C "$_proj" rev-parse origin/feature-1)
_result=$(cmd_add -b from-hash "$_hash" 2>&3)
_new_sha=$(git -C "$_proj" rev-parse from-hash)
assert_eq "$_hash" "$_new_sha"

it "handles branch name with slashes"
git -C "$_source" checkout -b features/add-wd 2>&3 >&3
echo "slash" >"$_source/slash.txt"
git -C "$_source" add -A 2>&3 >&3
git -C "$_source" commit -m "slash branch" 2>&3 >&3
git -C "$_proj" fetch origin 2>&3 >&3
_result=$(cmd_add features/add-wd 2>&3)
if [ -d "$_proj/wt-features-add-wd" ]; then _pass; else _fail "worktree not found"; fi

it "handles -b branch name with slashes"
_result=$(cmd_add -b feat/slashy 2>&3)
if [ -d "$_proj/wt-feat-slashy" ]; then _pass; else _fail "worktree not found"; fi

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

it "-b requires an argument"
assert_exit_code 1 cmd_add -b

it "fails with no arguments"
assert_exit_code 1 cmd_add

it "fails with unknown option"
assert_exit_code 1 cmd_add --unknown

it "fails with too many arguments"
assert_exit_code 1 cmd_add branch1 branch2

it "fails with -b and too many positional arguments"
assert_exit_code 1 cmd_add -b newbr base extra

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
describe "cmd_repair"
# ============================================================

setup_test_env

_source=$(create_local_bare_repo "$_test_tmpdir/source_repair")
_orig_parse_repo_url=$(type parse_repo_url | tail -n +2)
# shellcheck disable=SC2317
parse_repo_url() { echo "github.com/test/repair-test"; } # monkey-patch
_result=$(cmd_clone "$_source" 2>&3)
_proj="$WD_ROOT/github.com/test/repair-test"

cd "$_proj/main"

it "repairs paths broken by git worktree repair"
# Git v2.47: rewrites relative paths to absolute. cmd_repair restores them.
# Git v2.48+: may keep relative paths. cmd_repair is idempotent, so still passes.
git -C "$_proj" worktree repair 2>&3 >&3
cmd_repair 2>&3 >&3
_git_content=$(cat "$_proj/main/.git")
assert_eq "gitdir: ../.bare/worktrees/main" "$_git_content"
_gitdir_content=$(cat "$_proj/.bare/worktrees/main/gitdir")
assert_eq "../../../main/.git" "$_gitdir_content"

it "repairs absolute paths back to relative"
# Overwrite with absolute paths (simulating git worktree repair behavior)
echo "gitdir: $_proj/.bare/worktrees/main" >"$_proj/main/.git"
echo "$_proj/main/.git" >"$_proj/.bare/worktrees/main/gitdir"
cmd_repair 2>&3 >&3
_git_content=$(cat "$_proj/main/.git")
assert_eq "gitdir: ../.bare/worktrees/main" "$_git_content"
_gitdir_content=$(cat "$_proj/.bare/worktrees/main/gitdir")
assert_eq "../../../main/.git" "$_gitdir_content"

it "repairs multiple worktrees"
_result=$(cmd_add -b repair-multi 2>&3)
# Overwrite both with absolute paths
echo "gitdir: $_proj/.bare/worktrees/main" >"$_proj/main/.git"
echo "$_proj/main/.git" >"$_proj/.bare/worktrees/main/gitdir"
echo "gitdir: $_proj/.bare/worktrees/wt-repair-multi" >"$_proj/wt-repair-multi/.git"
echo "$_proj/wt-repair-multi/.git" >"$_proj/.bare/worktrees/wt-repair-multi/gitdir"
cmd_repair 2>&3 >&3
_git_content=$(cat "$_proj/wt-repair-multi/.git")
assert_eq "gitdir: ../.bare/worktrees/wt-repair-multi" "$_git_content"

it "skips worktree entry when directory is missing"
# Create a fake worktree entry with no matching directory
mkdir -p "$_proj/.bare/worktrees/ghost"
echo "fake" >"$_proj/.bare/worktrees/ghost/gitdir"
_output=$(cmd_repair 2>&3)
assert_not_contains "$_output" "ghost"

it "no output when worktrees directory does not exist"
rm -rf "$_proj/.bare/worktrees"
_output=$(cmd_repair 2>&3)
if [ -z "$_output" ]; then _pass; else _fail "expected no output, got: $_output"; fi

it "fails outside project"
cd "$_test_tmpdir"
assert_exit_code 1 cmd_repair

eval "$_orig_parse_repo_url"
unset _orig_parse_repo_url

teardown_test_env

# ============================================================

test_summary
