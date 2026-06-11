#!/bin/sh
set -eu

WD_VERSION="dev"
WD_ROOT="${WD_ROOT:-$HOME/Repositories}"
WD_TESTING="${WD_TESTING:-0}"

# --- utils ---

die() {
  echo "wd: error: $*" >&2
  exit 1
}

parse_repo_url() {
  _url="$1"
  case "$_url" in
  git@*:*/*)
    # git@github.com:owner/repo.git
    _host=$(echo "$_url" | sed 's/^git@\([^:]*\):.*/\1/')
    _path=$(echo "$_url" | sed 's/^git@[^:]*:\(.*\)/\1/; s/\.git$//')
    ;;
  *)
    die "Cannot parse repository URL: $_url"
    ;;
  esac
  echo "$_host/$_path"
  unset _url _host _path
}

find_project_root() {
  _dir="$(pwd)"
  while [ "$_dir" != "/" ]; do
    if [ -d "$_dir/.bare" ]; then
      echo "$_dir"
      unset _dir
      return
    fi
    _dir=$(dirname "$_dir")
  done
  unset _dir
  return 1
}

list_worktrees() {
  _lw_root="$1"
  git -C "$_lw_root" worktree list --porcelain | while IFS= read -r _lw_line; do
    case "$_lw_line" in
    worktree\ *)
      _lw_path="${_lw_line#worktree }"
      if [ "$_lw_path" != "$_lw_root" ] && [ "$_lw_path" != "$_lw_root/.bare" ]; then
        echo "${_lw_path##*/}"
      fi
      ;;
    esac
  done
  unset _lw_root
}

sanitize_branch() {
  echo "$1" | tr '/' '-'
}

write_relative_paths() {
  _wrp_root="$1"
  _wrp_name="$2"
  echo "gitdir: ../.bare/worktrees/$_wrp_name" >"$_wrp_root/$_wrp_name/.git"
  echo "../../../$_wrp_name/.git" >"$_wrp_root/.bare/worktrees/$_wrp_name/gitdir"
  unset _wrp_root _wrp_name
}

get_default_worktree() {
  _gd_root="$1"
  if [ -L "$_gd_root/.devcontainer" ]; then
    _gd_target=$(readlink "$_gd_root/.devcontainer")
    echo "$_gd_target" | sed 's|/.*||'
  else
    _gd_wts=$(list_worktrees "$_gd_root")
    for _gd_wt in $_gd_wts; do
      case "$_gd_wt" in
      wt-* | pr-*) ;;
      *)
        echo "$_gd_wt"
        break
        ;;
      esac
    done
  fi
  unset _gd_root _gd_target _gd_wts _gd_wt
}

# --- cmd_clone ---

cmd_clone() {
  _branch=""
  _url=""
  while [ $# -gt 0 ]; do
    case "$1" in
    -b | --branch)
      [ $# -ge 2 ] || die "wd clone: --branch requires an argument"
      _branch="$2"
      shift 2
      ;;
    -*)
      die "wd clone: unknown option: $1"
      ;;
    *)
      [ -z "$_url" ] || die "wd clone: too many arguments"
      _url="$1"
      shift
      ;;
    esac
  done
  [ -n "$_url" ] || {
    usage
    exit 1
  }

  _parsed=$(parse_repo_url "$_url")
  _project_dir="$WD_ROOT/$_parsed"

  [ ! -d "$_project_dir" ] || die "$_project_dir already exists"

  mkdir -p "$_project_dir"
  trap '[ $? -eq 0 ] || rm -rf "$_project_dir"' EXIT # when failed

  git clone --bare "$_url" "$_project_dir/.bare"
  echo "gitdir: ./.bare" >"$_project_dir/.git"
  git -C "$_project_dir" config remote.origin.fetch "+refs/heads/*:refs/remotes/origin/*"
  git -C "$_project_dir" fetch origin

  # resolve branch name
  _head_branch=""
  if _head_branch=$(git -C "$_project_dir" symbolic-ref HEAD 2>/dev/null); then
    _head_branch="${_head_branch#refs/heads/}"
  fi

  _default_branch=""
  if [ -n "$_branch" ]; then
    _default_branch="$_branch"
  elif [ -n "$_head_branch" ]; then
    _default_branch="$_head_branch"
  else
    die "Could not detect default branch. Please specify -b <branch>"
  fi

  # check if branch exists on remote
  _branch_exists=false
  if git -C "$_project_dir" show-ref --verify --quiet "refs/remotes/origin/$_default_branch" 2>/dev/null; then
    _branch_exists=true
  fi

  if [ -n "$_branch" ] && ! $_branch_exists && [ "$_branch" != "$_head_branch" ]; then
    die "branch '$_default_branch' not found"
  fi

  # create worktree
  _wt_name=$(sanitize_branch "$_default_branch")
  if $_branch_exists; then
    git -C "$_project_dir" worktree add "$_wt_name" "$_default_branch"
  else
    git -C "$_project_dir" worktree add --orphan -b "$_default_branch" "$_wt_name"
  fi
  write_relative_paths "$_project_dir" "$_wt_name"

  # symlink .devcontainer if present
  if [ -d "$_project_dir/$_wt_name/.devcontainer" ]; then
    ln -s "$_wt_name/.devcontainer" "$_project_dir/.devcontainer"
  fi

  echo "$_project_dir"
  unset _branch _url _parsed _project_dir _head_branch _default_branch _branch_exists _wt_name
}

# --- cmd_list ---

cmd_list() {
  _cl_full_path=false
  _cl_worktrees=false
  while [ $# -gt 0 ]; do
    case "$1" in
    --full-path)
      _cl_full_path=true
      shift
      ;;
    --worktrees)
      _cl_worktrees=true
      shift
      ;;
    -*)
      die "wd list: unknown option: $1"
      ;;
    *)
      die "wd list: too many arguments"
      ;;
    esac
  done

  for _proj_dir in "$WD_ROOT"/*/*/*; do
    if [ ! -d "$_proj_dir/.bare" ]; then
      continue
    fi

    if $_cl_full_path; then
      _display="$_proj_dir"
    else
      _display="${_proj_dir#"$WD_ROOT"/}"
    fi

    if $_cl_worktrees; then
      _wt_names=$(list_worktrees "$_proj_dir")
      for _t in $_wt_names; do
        echo "$_display/$_t"
      done
    else
      echo "$_display"
    fi
  done
  unset _cl_full_path _cl_worktrees
}

# --- cmd_add ---

cmd_add() {
  _new_branch=""
  _create=false
  _commitish=""
  _pr_number=""
  while [ $# -gt 0 ]; do
    case "$1" in
    -b | --branch)
      [ $# -ge 2 ] || die "wd add: --branch requires an argument"
      _create=true
      _new_branch="$2"
      shift 2
      ;;
    --pr)
      [ $# -ge 2 ] || die "wd add: --pr requires an argument"
      _pr_number="$2"
      shift 2
      ;;
    -*)
      die "wd add: unknown option: $1"
      ;;
    *)
      [ -z "$_commitish" ] || die "wd add: too many arguments"
      _commitish="$1"
      shift
      ;;
    esac
  done

  _project_dir=$(find_project_root) || die "not in a wd project"

  if [ -n "$_pr_number" ]; then
    _wt_dir="pr-$_pr_number"
    [ ! -d "$_project_dir/$_wt_dir" ] || die "worktree '$_wt_dir' already exists"
    git -C "$_project_dir" fetch origin "pull/$_pr_number/head:$_wt_dir"
    git -C "$_project_dir" worktree add "$_wt_dir" "$_wt_dir"
    write_relative_paths "$_project_dir" "$_wt_dir"
  elif $_create; then
    _wt_dir="wt-$(sanitize_branch "$_new_branch")"
    [ ! -d "$_project_dir/$_wt_dir" ] || die "worktree '$_new_branch' already exists"
    if [ -n "$_commitish" ]; then
      git -C "$_project_dir" worktree add "$_wt_dir" -b "$_new_branch" "$_commitish"
    else
      git -C "$_project_dir" worktree add "$_wt_dir" -b "$_new_branch"
    fi
    write_relative_paths "$_project_dir" "$_wt_dir"
  else
    [ -n "$_commitish" ] || {
      usage
      exit 1
    }
    _wt_dir="wt-$(sanitize_branch "$_commitish")"
    [ ! -d "$_project_dir/$_wt_dir" ] || die "worktree '$_commitish' already exists"
    git -C "$_project_dir" worktree add "$_wt_dir" "$_commitish"
    write_relative_paths "$_project_dir" "$_wt_dir"
  fi

  echo "$_project_dir/$_wt_dir"
  unset _new_branch _create _commitish _pr_number _project_dir _wt_dir
}

# --- cmd_remove ---

_remove_one() {
  _ro_root="$1"
  _ro_name="$2"
  _ro_delete_branch="$3"

  _ro_branch=""
  if $_ro_delete_branch; then
    _ro_branch=$(git -C "$_ro_root/$_ro_name" symbolic-ref --short HEAD 2>/dev/null) || true
  fi

  git -C "$_ro_root" worktree remove "$_ro_name"
  echo "Removed worktree '$_ro_name'"

  if [ -n "$_ro_branch" ]; then
    if git -C "$_ro_root" branch -D "$_ro_branch" 2>/dev/null; then
      echo "Deleted branch '$_ro_branch'"
    else
      echo "wd: warning: failed to delete branch '$_ro_branch'" >&2
    fi
  fi
  unset _ro_root _ro_name _ro_delete_branch _ro_branch
}

cmd_remove() {
  _delete_branch=false
  _remove_all=false
  _targets=""
  while [ $# -gt 0 ]; do
    case "$1" in
    -b | --branch)
      _delete_branch=true
      shift
      ;;
    -a)
      _remove_all=true
      shift
      ;;
    -*)
      die "wd remove: unknown option: $1"
      ;;
    *)
      _targets="$_targets $1"
      shift
      ;;
    esac
  done

  _project_dir=$(find_project_root) || die "not in a wd project"
  _default_wt=$(get_default_worktree "$_project_dir")

  if $_remove_all; then
    _targets=$(list_worktrees "$_project_dir")
  elif [ -z "$_targets" ]; then
    die "wd remove: worktree name required (or use -a to remove all)"
  fi

  for _wt in $_targets; do
    if [ "$_wt" = "$_default_wt" ]; then
      if ! $_remove_all; then
        die "cannot remove default worktree '$_wt'"
      fi
      continue
    fi
    if [ ! -d "$_project_dir/$_wt" ]; then
      die "worktree '$_wt' does not exist"
    fi
    _remove_one "$_project_dir" "$_wt" "$_delete_branch"
  done
  unset _delete_branch _remove_all _targets _project_dir _default_wt _wt
}

# --- cmd_repair ---

cmd_repair() {
  _project_dir=$(find_project_root) || die "not in a wd project"
  _rp_wt_dir="$_project_dir/.bare/worktrees"
  if [ ! -d "$_rp_wt_dir" ]; then
    unset _project_dir _rp_wt_dir
    return
  fi
  for _rp_entry in "$_rp_wt_dir"/*/; do
    if [ ! -d "$_rp_entry" ]; then
      continue
    fi
    _rp_name="${_rp_entry%/}"
    _rp_name="${_rp_name##*/}"
    if [ -d "$_project_dir/$_rp_name" ]; then
      write_relative_paths "$_project_dir" "$_rp_name"
      echo "Repaired worktree '$_rp_name'"
    fi
  done
  unset _project_dir _rp_wt_dir _rp_entry _rp_name
}

# --- cmd_update ---

cmd_update() {
  _ud_url="https://github.com/shokkunrf/wd/releases/latest/download/install.sh"
  if command -v curl >/dev/null 2>&1; then
    curl -fsSL "$_ud_url" | sh
  elif command -v wget >/dev/null 2>&1; then
    wget -qO- "$_ud_url" | sh
  else
    die "curl or wget is required"
  fi
  unset _ud_url
}

# --- usage ---

usage() {
  cat <<'EOF'
wd - git worktree manager with devcontainer support

Usage: wd <command> [options]

Project Management:
  clone <repo-url> [-b <branch>]  Clone repository (bare + worktree)
  list [--full-path] [--worktrees]  List managed projects

Worktree Management:
  add <branch>                    Add worktree from a branch or commit-ish
  add -b <branch> [<base>]        Add worktree, creating <branch> from <base>
  add --pr <number>               Add PR review worktree
  remove <name>... [-b] [--branch]  Remove worktree(s) and optionally branch
  remove -a [-b] [--branch]       Remove all non-default worktrees
  repair                          Repair worktree relative paths

General:
  update                          Update wd to the latest version

Options:
  --version    Show version
  --help       Show this help

Environment:
  WD_ROOT    Root directory (default: ~/Repositories)
EOF
}

# --- main ---

main() {
  command="${1:-}"

  case "$command" in
  clone)
    shift
    cmd_clone "$@"
    ;;
  list)
    shift
    cmd_list "$@"
    ;;
  add)
    shift
    cmd_add "$@"
    ;;
  remove)
    shift
    cmd_remove "$@"
    ;;
  repair)
    shift
    cmd_repair "$@"
    ;;
  update)
    shift
    cmd_update "$@"
    ;;
  --version | -v) echo "wd version $WD_VERSION" ;;
  --help | -h | "") usage ;;
  *)
    echo "wd: error: Unknown command: $command. Run 'wd --help' for usage." >&2
    exit 1
    ;;
  esac
}

if [ "$WD_TESTING" != "1" ]; then
  main "$@"
fi
