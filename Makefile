# Signing with a fixed identity keeps macOS privacy permissions across rebuilds.
VERSION := $(shell sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml)
BIN := target/release/tinylogi
APP := target/Tinylogi.app
ZIP := target/Tinylogi-$(VERSION).zip
SIGN := codesign -f -s tinylogi -i net.caltona.tinylogi
TAG := v$(VERSION)
TAP ?= ../homebrew-tap
CASK := $(TAP)/Casks/tinylogi.rb

.PHONY: build app dist release dev log

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

# Tags the current commit as v$(VERSION) from Cargo.toml, publishes the zip as a GitHub release,
# then points the cask in $(TAP) at it. Bump the version in Cargo.toml and commit first.
release:
	@test -z "$$(git status --porcelain)" || { echo "release: commit your changes first"; exit 1; }
	@test -z "$$(git -C $(TAP) status --porcelain)" || { echo "release: $(TAP) has uncommitted changes"; exit 1; }
	@! git rev-parse -q --verify $(TAG) >/dev/null || { echo "release: $(TAG) exists, bump the version"; exit 1; }
	$(MAKE) dist
	git tag -a $(TAG) -m "tinylogi $(VERSION)"
	git push origin HEAD $(TAG)
	gh release create $(TAG) $(ZIP) --title "tinylogi $(VERSION)" --generate-notes
	git -C $(TAP) pull -q --ff-only
	sed -i '' -e 's/^  version ".*"/  version "$(VERSION)"/' \
		-e "s/^  sha256 \".*\"/  sha256 \"$$(shasum -a 256 $(ZIP) | cut -d' ' -f1)\"/" $(CASK)
	git -C $(TAP) commit -qam "tinylogi $(VERSION)"
	git -C $(TAP) push -q

# Runs the current build in the foreground with debug logging, pausing the installed app.
dev: app
	macos/dev.sh $(CURDIR)/$(APP)

# Follows the installed (Homebrew) app's log.
log:
	tail -f /tmp/tinylogi.log
