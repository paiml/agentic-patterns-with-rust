#!/usr/bin/env bash
# Cross-check the generated README against the TOML it was rendered from.
#
# This is a CONSISTENCY CHECK, not a regenerate-and-diff. It catches the
# realistic hand-edit -- a reworded title or tagline, an added or removed table
# row, a deleted footer, a link to a directory that is not there -- because
# those disagree with the data. It does NOT catch prose reworded in both the
# TOML and the README at once; only running the real generator would see that.
#
# This course is at the outline stage: course.toml may list no [[examples]]
# yet. Then the README must carry no Examples table at all -- never an empty
# one -- and OUTLINE.md must exist, because the outline is what this repository
# holds until the first example lands.
#
# --self-test breaks deliberate copies of this tree and asserts each break is
# reported, and that a clean tree passes. A checker nobody has watched fail is
# not evidence.
set -uo pipefail

if [ "${1:-}" = "--self-test" ]; then
    SRC="$(cd "$(dirname "$0")/../.." && pwd)"
    T=$(mktemp -d); trap 'rm -rf -- "${T:?}"' EXIT
    fails=0
    t(){ [ "$2" = "$3" ] && echo "  ok   $1" || { echo "  FAIL $1 (got $2 want $3)"; fails=$((fails+1)); }; }
    fresh(){ rm -rf "${T:?}/r"; mkdir -p "$T/r"; (cd "$SRC" && tar cf - --exclude=.git --exclude=target .) | (cd "$T/r" && tar xf -); }
    run(){ ROOT="$T/r" bash "$SRC/.github/scripts/check-readme-consistency.sh" >/dev/null 2>&1; echo $?; }

    fresh; t "S0 a clean tree PASSES"                          "$(run)" 0
    fresh; sed -i '0,/^# /s/^# .*/# Something Else Entirely/' "$T/r/README.md"
           t "S1 a reworded course README title is caught"      "$(run)" 1
    fresh; printf '| [`examples/ghost`](examples/ghost) | invented |\n' >> "$T/r/README.md"
           t "S2 an extra Examples table row is caught"         "$(run)" 1
    fresh; sed -i '/^<sub>Generated/d' "$T/r/README.md"
           t "S3 a deleted generated-file footer is caught"     "$(run)" 1
    fresh; sed -i 's/^\(tagline *= *"\)/\1Reworded /' "$T/r/course.toml"
           t "S4 a tagline that disagrees with the README is caught" "$(run)" 1
    fresh; rm -f "${T:?}/r/OUTLINE.md"
           t "S5 a missing OUTLINE.md is caught"                "$(run)" 1
    fresh; mkdir -p "$T/r/examples/orphan" && printf '# Orphan\n' > "$T/r/examples/orphan/README.md"
           t "S6 a README no TOML claims is caught"             "$(run)" 1
    fresh; printf '\n[[examples]]\ndir     = "examples/gone"\ntitle   = "Gone"\nsummary = "not on disk"\n' >> "$T/r/course.toml"
           t "S7 a course.toml pointing at a missing dir is caught" "$(run)" 1
    fresh; rm -f "${T:?}/r/course.toml"
           t "S8 nothing to check is caught"                    "$(run)" 1

    echo "self-test: $((9-fails))/9 passed"
    [ "$fails" -eq 0 ] || exit 1
    exit 0
fi
ROOT="${ROOT:-$(cd "$(dirname "$0")/../.." && pwd)}"
G=/usr/bin/grep
problems=0
bad(){ printf '  FAIL  %s\n        %s\n' "$1" "$2"; problems=$((problems+1)); }

# crude but sufficient TOML readers for the shapes this repo uses
field(){ "$G" -m1 "^ *$2 *=" "$1" 2>/dev/null | sed 's/^[^=]*= *//; s/^"//; s/"$//'; }
arr_dirs(){ "$G" -A4 '^\[\[examples\]\]' "$1" 2>/dev/null | "$G" '^ *dir *=' | sed 's/^[^=]*= *//; s/"//g'; }
file_paths(){ "$G" -A3 '^\[\[files\]\]' "$1" 2>/dev/null | "$G" '^ *path *=' | sed 's/^[^=]*= *//; s/"//g'; }

checked_readmes=0
checked_examples=0
declare -a claimed=()

# ---- course level -----------------------------------------------------------
CT="$ROOT/course.toml"; CR="$ROOT/README.md"
[ -f "$CT" ] || { echo "  FAIL  anti-vacuity: course.toml is missing, so there is nothing to check"; exit 1; }
[ -f "$CR" ] || bad "README.md" "missing"
for f in slug title tagline description; do
  [ -n "$(field "$CT" "$f")" ] || bad "course.toml" "[course].$f is missing or empty"
done
n_toml=$(arr_dirs "$CT" | "$G" -c . || true)
if [ -f "$CR" ]; then
  checked_readmes=$((checked_readmes+1)); claimed+=("README.md")
  t=$(field "$CT" title)
  [ "$("$G" -m1 '^# ' "$CR")" = "# $t" ] || bad "README.md" "title does not match course.toml ([course].title = \"$t\")"
  tl=$(field "$CT" tagline)
  "$G" -qFx -- "$tl" "$CR" || bad "README.md" "tagline does not match course.toml ([course].tagline = \"$tl\")"
  "$G" -q '^<sub>Generated' "$CR" || bad "README.md" "generated-file footer is missing"
  n_rows=$("$G" -c '^| \[`examples/' "$CR" || true)
  [ "$n_toml" = "$n_rows" ] || bad "README.md" "Examples table has $n_rows row(s), course.toml has $n_toml [[examples]] entries"
  if [ "$n_toml" = 0 ] && "$G" -q '^## Examples' "$CR"; then
    bad "README.md" "course.toml has no [[examples]] but the README has an Examples heading"
  fi
fi
[ -f "$ROOT/OUTLINE.md" ] || bad "OUTLINE.md" "missing -- the outline is what this repository holds at this stage"

# ---- example level ----------------------------------------------------------
while read -r d; do
  [ -z "$d" ] && continue
  [ -d "$ROOT/$d" ] || { bad "course.toml" "[[examples]] points at $d, which does not exist"; continue; }
  ET="$ROOT/$d/example.toml"; ER="$ROOT/$d/README.md"
  [ -f "$ET" ] || { bad "course.toml" "$d has no example.toml"; continue; }
  checked_examples=$((checked_examples+1))
  for f in title summary; do
    [ -n "$(field "$ET" "$f")" ] || bad "$d/example.toml" "[example].$f is missing or empty"
  done
  [ -f "$ER" ] || { bad "$d" "README.md is missing"; continue; }
  checked_readmes=$((checked_readmes+1)); claimed+=("$d/README.md")
  t=$(field "$ET" title)
  "$G" -qF "# $t" "$ER" || bad "$d/README.md" "title does not match example.toml ([example].title = \"$t\")"
  "$G" -q '^<sub>Generated' "$ER" || bad "$d/README.md" "generated-file footer is missing"
  while read -r p; do
    [ -z "$p" ] && continue
    [ -e "$ROOT/$d/$p" ] || bad "$d/example.toml" "[[files]] lists $p, which is not on disk"
    "$G" -qF "\`$p\`" "$ER" || bad "$d/README.md" "does not mention $p, which example.toml lists"
  done < <(file_paths "$ET")
done < <(arr_dirs "$CT")

# ---- anti-vacuity -----------------------------------------------------------
# Without these a checker that discovers nothing reports success.
[ "$checked_readmes" -ge 1 ] || bad "anti-vacuity" "no README was checked"
[ "$checked_examples" = "$n_toml" ] || bad "anti-vacuity" "course.toml lists $n_toml example(s) but $checked_examples were checked"
while read -r r; do
  rel="${r#"$ROOT"/}"
  found=0; for c in "${claimed[@]}"; do [ "$c" = "$rel" ] && found=1; done
  [ "$found" = 1 ] || bad "anti-vacuity" "$rel is a README no TOML in this repo claims, so nothing above checked it"
done < <(find "$ROOT" -name README.md -not -path '*/.git/*' -not -path '*/target/*')

if [ "$problems" -eq 0 ]; then
  echo "README consistency OK — $checked_readmes README(s), $checked_examples example(s)"
  exit 0
fi
echo; echo "$problems problem(s)."; exit 1
