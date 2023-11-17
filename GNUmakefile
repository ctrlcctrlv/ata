all: README.md ata

README.md: README.md.sh
	./$< > $@

.PHONY: ata
ata:
	cargo build --release

.PHONY: install
install:
	cargo install --path ./ata²
