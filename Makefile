APP_NAME := minesweep_rs
PREFIX ?= /usr/local
BINDIR := $(PREFIX)/bin
DATADIR := $(PREFIX)/share

.PHONY: build install uninstall clean

build:
	cargo build --release

install: build
	install -Dm755 target/release/$(APP_NAME) $(DESTDIR)$(BINDIR)/$(APP_NAME)

	install -Dm644 $(APP_NAME).desktop $(DESTDIR)$(DATADIR)/applications/$(APP_NAME).desktop

	install -Dm644 gui/assets/icon.svg $(DESTDIR)$(DATADIR)/icons/hicolor/scalable/apps/$(APP_NAME).svg

	install -Dm644 gui/assets/icon.png $(DESTDIR)$(DATADIR)/icons/hicolor/256x256/apps/$(APP_NAME).png

	-gtk-update-icon-cache -f -t $(DESTDIR)$(DATADIR)/icons/hicolor 2>/dev/null || true
	-update-desktop-database $(DESTDIR)$(DATADIR)/applications 2>/dev/null || true

uninstall:
	rm -f $(DESTDIR)$(BINDIR)/$(APP_NAME)
	rm -f $(DESTDIR)$(DATADIR)/applications/$(APP_NAME).desktop
	rm -f $(DESTDIR)$(DATADIR)/icons/hicolor/scalable/apps/$(APP_NAME).svg
	rm -f $(DESTDIR)$(DATADIR)/icons/hicolor/256x256/apps/$(APP_NAME).png

clean:
	cargo clean
