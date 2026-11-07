# Signing with a fixed identity keeps macOS privacy permissions across rebuilds.
BIN := target/release/tinylogi
AGENT := gui/$(shell id -u)/net.caltona.tinylogi

.PHONY: build restart log

# Sign a fresh copy and move it into place: macOS caches signatures per file, and a binary
# rewritten in place gets killed at launch (OS_REASON_CODESIGNING).
build:
	cargo build --release
	cp $(BIN) $(BIN).new
	codesign -f -s tinylogi -i net.caltona.tinylogi $(BIN).new
	mv -f $(BIN).new $(BIN)

restart: build
	launchctl kickstart -k $(AGENT)

log:
	tail -f /tmp/tinylogi.log
