#!/bin/sh
# test_unit.sh — unit tests for wd
set -eu

cd "$(dirname "$0")/.."
. tests/test_helpers.sh

WD_TESTING=1
export WD_TESTING
. src/wd.sh

# ============================================================
describe "parse_repo_url"
# ============================================================

it "parses git@ SSH URL"
_result=$(parse_repo_url "git@github.com:owner/repo.git")
assert_eq "github.com/owner/repo" "$_result"

it "parses git@ SSH URL without .git"
_result=$(parse_repo_url "git@github.com:owner/repo")
assert_eq "github.com/owner/repo" "$_result"

it "parses custom host SSH URL"
_result=$(parse_repo_url "git@gitlab.com:owner/repo.git")
assert_eq "gitlab.com/owner/repo" "$_result"

it "rejects non-SSH URL"
assert_exit_code 1 parse_repo_url "https://github.com/owner/repo"

it "rejects invalid input"
assert_exit_code 1 parse_repo_url "invalid"

# ============================================================
describe "main dispatch"
# ============================================================

it "shows version with --version"
_result=$(main --version)
assert_contains "$_result" "wd version"

it "shows help with --help"
_result=$(main --help)
assert_contains "$_result" "Usage:"

it "shows help with no arguments"
_result=$(main "")
assert_contains "$_result" "Usage:"

it "exits with error for unknown command"
assert_exit_code 1 main "nonexistent"

# ============================================================

test_summary
