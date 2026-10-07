# modules-lab — the gates of the views here. Every wasm artifact is a build
# output under $(BUILD_TARGET); none is committed.
CARGO ?= cargo
WASM_OPT ?= wasm-opt
VIEWS := market-view

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

.PHONY: test wasm-views shots

## fmt, clippy and the native tests: each view on a fake host.
test:
	$(CARGO) fmt --all -- --check
	$(CARGO) clippy --workspace --all-targets -- -D warnings
	$(CARGO) test --workspace

## builds every view for wasm32 under $(RELEASE)/, optimized and ABI-checked
## as modules gates its own, and prints `name  bytes`.
wasm-views:
	@$(foreach v,$(VIEWS),RUSTFLAGS="$(WASM_RUSTFLAGS)" $(CARGO) build --release --target wasm32-unknown-unknown -p $(v) && WASM_OPT=$(WASM_OPT) sh $(SDK_TOOLS)/view-gate.sh $(v) $(RELEASE)/$(subst -,_,$(v)).wasm || exit 1;)

## a view with no node: writes the frames market-view draws and has the
## app's real renderer paint them into shots/*.png (Linux: Xvfb, xdotool,
## ImageMagick; see $(APP)/dev/screens/README.md).
shots:
	$(CARGO) run -q -p market-view --example frames -- shots/fixtures
	CARGO_TARGET_DIR=$(abspath $(APP_TARGET)) $(APP)/dev/screens/chat-screens.sh shots/fixtures shots
