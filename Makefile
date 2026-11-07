# Signing with a fixed identity keeps macOS privacy permissions across rebuilds.
VERSION := $(shell sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml)
BIN := target/release/tinylogi
APP := target/Tinylogi.app
ZIP := target/Tinylogi-$(VERSION).zip
SIGN := codesign -f -s tinylogi -i net.caltona.tinylogi
AGENT := gui/$(shell id -u)/net.caltona.tinylogi

.PHONY: build app dist restart log

# Sign a fresh copy and move it into place: macOS caches signatures per file, and a binary
# rewritten in place gets killed at launch (OS_REASON_CODESIGNING).
build:
	cargo build --release
	cp $(BIN) $(BIN).new
	$(SIGN) $(BIN).new
	mv -f $(BIN).new $(BIN)

app: build
	rm -rf $(APP)
	mkdir -p $(APP)/Contents/MacOS
	sed 's/VERSION/$(VERSION)/' macos/Info.plist > $(APP)/Contents/Info.plist
	cp $(BIN) $(APP)/Contents/MacOS/tinylogi
	$(SIGN) $(APP)

# Release asset for the Homebrew cask: the app plus its LaunchAgent and example config.
dist: app
	rm -rf target/dist $(ZIP)
	mkdir -p target/dist
	cp -R $(APP) macos/net.caltona.tinylogi.plist tinylogi.example.toml target/dist/
	ditto -c -k --norsrc --noextattr --noacl target/dist $(ZIP)
	shasum -a 256 $(ZIP)

restart: build
	launchctl kickstart -k $(AGENT)

log:
	tail -f /tmp/tinylogi.log
