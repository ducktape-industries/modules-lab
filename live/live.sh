#!/bin/sh
# The workspace's modules in the real app, from a clone. `make live` and `make live-down`
# run this, and the Makefile passes every variable it reads.
#
#   live.sh up     builds each program, its describe module and its view, packs them into
#                  one <program>.wasm, founds stage $STAGE of qa's kit with them (trade
#                  with a week of every market's minutes), and seeds trade: two accounts
#                  with their funds, 36 orders of one resting around BTC/USD's last price,
#                  two market orders of the other's that trade. Prints the command that
#                  opens the app on it. Run again, it founds a fresh network on the stage.
#   live.sh down   stops the stage's node, which runs until then
#
# qa is not edited. kit founds from the founding.toml of its root, so the root is the
# lab's own ($OUT: qa's founding with live/founding.toml after it), and its .local is
# qa's, where kit keeps the stages, the worktrees and the builds.
set -eu

die() {
  echo "live: $*" >&2
  exit 1
}

# The workspace's root: Cargo.toml and the make targets are here, and a relative QA is
# from here.
cd "$(dirname "$0")/.."

[ -n "$QA" ] || die "QA=<a qa checkout> is needed: kit is qa's, and core, app and modules are checked out beside it"
[ -f "$QA/founding.toml" ] || die "$QA is not a qa checkout: it has no founding.toml"
QA=$(cd "$QA" && pwd -P)
KIT=${KIT:-$QA/kit}
dir=$OUT/.local/stages/$STAGE

case "${1:-}" in
up) ;;
down)
  "$KIT" down "$STAGE" --root "$QA"
  exit
  ;;
*)
  sed -n 2,15p live/live.sh
  exit 2
  ;;
esac

for tool in cargo git python3 wasm-opt wasm-tools ps; do
  command -v "$tool" >/dev/null || die "$tool is not on PATH (the README names what a live stage needs)"
done

# The modules rev a Cargo.toml pins.
pin() { sed -n 's|.*ducktape-industries/modules", rev = "\([0-9a-f]*\)".*|\1|p' "$1" | sort -u; }
modules=$(pin Cargo.toml)
case $modules in
'' | *[!0-9a-f]*) die "Cargo.toml does not pin one modules rev (its sdk-line)" ;;
esac

mkdir -p "$OUT"
cat "$QA/founding.toml" live/founding.toml >"$OUT/founding.toml"
ln -sfn "$QA/.local" "$OUT/.local"

# The stage, at this workspace's modules. kit makes a ref's worktree from the checkout
# beside its root, so this one call has qa as its root.
set -- new "$STAGE" --root "$QA" --modules "$modules"
[ -z "$APP_REF" ] || set -- "$@" --app "$APP_REF"
[ -z "$CORE_REF" ] || set -- "$@" --core "$CORE_REF"
"$KIT" "$@"

# The app refuses a view built on another SDK line: its checkout must pin the same rev.
app=$(python3 -c 'import json, sys; print(json.load(open(sys.argv[1]))["app"]["path"])' "$dir/stage.json")
pinned=$(pin "$app/Cargo.toml")
[ "$pinned" = "$modules" ] || die "the app at ${APP_REF:-its working checkout} pins modules ${pinned:-none}
and this workspace pins modules $modules.
The app refuses a view built on another SDK line: pass APP_REF=<a ref of the app that pins $modules>."

"$MAKE" wasm-programs wasm-describes wasm-views
cargo run -q -p trade --example history -- "$OUT/trade.params"
cargo run -q -p trade --example seed -- "$OUT/trade.params" "$OUT/seed"

# The node, the app and kit's own programs. `up` packs those only for a stage that has
# none, so a stage made anew at other refs is packed again here.
"$KIT" build "$STAGE" --root "$OUT"

# Each program as a founding takes it: its view (<program>-view), then its describe
# module, packed in.
pack() { cargo run -q -p view-pack --manifest-path "$QA/Cargo.toml" -- "$@"; }
for program in $PROGRAMS; do
  name=$(echo "$program" | tr - _)
  pack "$RELEASE/$name.wasm" "$RELEASE/${name}_view.wasm" "$dir/programs/$name.wasm"
  pack --describe "$dir/programs/$name.wasm" "$RELEASE/$name.describe.wasm" "$dir/programs/$name.wasm"
done
cp "$OUT/trade.params" "$dir/programs/trade.params"

"$KIT" up "$STAGE" --root "$OUT"

# kit's CLI signs as wallet `dev` alone, and the book's maker is another wallet. So the
# seed goes through the node's own binary, the one kit started: WALLET ARGS...
node=$(ps -ww -p "$(cat "$dir/node.pid")" -o args= | sed 's| run --listen .*||')
[ -x "$node" ] || die "the node kit started is not running: see $dir/node.log"
as() {
  key=$dir/home/keys/$1.key
  shift
  DUCKTAPE_HOME="$dir/home" DUCKTAPE_PASSWORD="$(cat "$dir/password")" DUCKTAPE_USER_KEY="$key" "$node" "$@"
}
# WALLET PROGRAM OP: a refused op is exit 0 too, so the answer is read.
submit() {
  answer=$(as "$1" submit "$2" "$OUT/seed/$3")
  case $answer in
  applied*) ;;
  *) die "$1's $3 was not applied: $answer" ;;
  esac
}

echo "seeding stage $STAGE"
as dev wallet new bo >/dev/null
submit dev identity account-ann.bin
submit dev trade fund.bin
submit bo identity account-bo.bin
submit bo trade fund.bin
for op in "$OUT"/seed/rest-*.bin; do submit bo trade "${op##*/}"; done
for op in "$OUT"/seed/take-*.bin; do submit dev trade "${op##*/}"; done

cat <<EOF

$PROGRAMS live on stage $STAGE: wallet dev has an account, its funds and two orders
that traded, and wallet bo's orders rest on both sides of BTC/USD.

  open the app    $KIT app $STAGE --root $OUT
                  (the wallet's password is $(cat "$dir/password"); each view is on the rail)
  stop the node   make live-down QA=$QA STAGE=$STAGE
                  (it runs until then)
EOF
