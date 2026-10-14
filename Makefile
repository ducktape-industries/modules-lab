# modules-lab — the gates of the programs and views here. Every wasm artifact is a build
# output under $(BUILD_TARGET); none is committed.
CARGO ?= cargo
WASM_OPT ?= wasm-opt
PROGRAMS := trade arcade live-echo
VIEWS := trade-view arcade-view browser-view live-echo-view

BUILD_TARGET := $(abspath $(or $(CARGO_TARGET_DIR),target))
RELEASE := $(BUILD_TARGET)/wasm32-unknown-unknown/release

# The view gate (wasm-opt, then the ABI check) is modules' own, run out of
# the SDK checkout cargo fetched at the pinned rev.
SDK_TOOLS = $(shell $(CARGO) metadata --format-version 1 | python3 -c 'import json, os, sys; print(os.path.join(next(os.path.dirname(p["manifest_path"]) for p in json.load(sys.stdin)["packages"] if p["name"] == "ducktape-view-guest"), "..", "..", "..", "tools"))')

# An app checkout at the rev that pins the same SDK line, and the debug
# build of it that holds `--render-tree`.
APP ?= ../app
APP_TARGET ?= $(APP)/target

# The same bytes from any checkout: panic locations would otherwise carry
# this checkout's, cargo's and the toolchain's absolute paths.
CARGO_HOME_DIR := $(or $(CARGO_HOME),$(HOME)/.cargo)
SYSROOT := $(shell rustc --print sysroot)
WASM_RUSTFLAGS := --remap-path-prefix=$(CURDIR)=/build --remap-path-prefix=$(CARGO_HOME_DIR)=/cargo --remap-path-prefix=$(SYSROOT)=/rustc

WASM_BUILD := RUSTFLAGS="$(WASM_RUSTFLAGS)" $(CARGO) build --release --target wasm32-unknown-unknown

# A program is a cdylib its view also links, with `module` off; in one
# target dir each build would rebuild the crate for the other. So a program
# builds in a target dir of its own, and its describe module in another.
program_wasm = $(BUILD_TARGET)/$1/$2/wasm32-unknown-unknown/release/$(subst -,_,$2).wasm

# A live stage is qa's kit's (`make live`): QA is a qa checkout, with core, app
# and modules checked out beside it, and KIT its kit (default: $(QA)/kit,
# which builds it first). The stage is STAGE, its modules the rev this
# workspace pins, its app APP_REF (it must pin that same rev) and its core
# CORE_REF (none: core's working checkout).
QA ?=
KIT ?=
STAGE ?= lab
APP_REF ?= dev
CORE_REF ?=
LIVE = PROGRAMS="$(PROGRAMS)" QA="$(QA)" KIT="$(KIT)" STAGE="$(STAGE)" APP_REF="$(APP_REF)" CORE_REF="$(CORE_REF)" OUT="$(BUILD_TARGET)/live" RELEASE="$(RELEASE)" MAKE="$(MAKE)" sh live/live.sh

.PHONY: test wasm-programs wasm-describes wasm-views shots live live-down bot

## fmt, clippy and the native tests: each view on a fake host.
test:
	$(CARGO) fmt --all -- --check
	$(CARGO) clippy --workspace --all-targets -- -D warnings
	$(CARGO) test --workspace

## builds every program for wasm32 into $(RELEASE)/<name>.wasm, unpacked:
## packing its view and describe module into it is the founding's job
## (qa's `view-pack`).
wasm-programs:
	@mkdir -p $(RELEASE)
	@$(foreach p,$(PROGRAMS),$(WASM_BUILD) --target-dir $(BUILD_TARGET)/programs/$(p) -p $(p) --features module && cp $(call program_wasm,programs,$(p)) $(RELEASE)/ || exit 1;)

## builds every program's describe module (the `alloc` and `describe`
## exports, no imports) into $(RELEASE)/<name>.describe.wasm, ABI-checked.
wasm-describes:
	@mkdir -p $(RELEASE)
	@$(foreach p,$(PROGRAMS),$(WASM_BUILD) --target-dir $(BUILD_TARGET)/describe/$(p) -p $(p) --features describe && cp $(call program_wasm,describe,$(p)) $(RELEASE)/$(subst -,_,$(p)).describe.wasm && python3 $(SDK_TOOLS)/check-describe-abi.py $(RELEASE)/$(subst -,_,$(p)).describe.wasm || exit 1;)

## builds every view for wasm32 under $(RELEASE)/, optimized and ABI-checked
## as modules gates its own, and prints `name  bytes`.
wasm-views:
	@$(foreach v,$(VIEWS),$(WASM_BUILD) -p $(v) && WASM_OPT=$(WASM_OPT) sh $(SDK_TOOLS)/view-gate.sh $(v) $(RELEASE)/$(subst -,_,$(v)).wasm || exit 1;)

## a view with no node: writes the frames trade-view draws and has the
## app's real renderer paint them into shots/*.png (Linux: Xvfb, xdotool,
## ImageMagick; see $(APP)/dev/screens/README.md).
shots:
	$(CARGO) run -q -p trade-view --example frames -- shots/fixtures
	CARGO_TARGET_DIR=$(abspath $(APP_TARGET)) $(APP)/dev/screens/chat-screens.sh shots/fixtures shots

## every module here in the real app: builds and packs each, founds stage
## $(STAGE) of qa's kit with them (trade with a week of history), seeds two
## accounts, a book and two orders that trade, and prints the command that
## opens the app on it. The stage's node runs until `make live-down`.
live:
	@$(LIVE) up

## stops that node, and the bot when it runs.
live-down:
	@$(LIVE) down

## the market maker on the stage `make live` made, until Ctrl-C.
bot:
	@$(LIVE) bot
