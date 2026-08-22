# Fuzz targets

`inventory` exercises the public probe, inventory, and parse byte entry points
under a deliberately small service-style resource budget. It also wraps each
bounded fuzz input as a valid modern `docProps/custom.xml` frame so mutations
reach the XML/property decoder directly.

Run a bounded local smoke with:

```bash
cargo +nightly fuzz run inventory -- -runs=10000
```

Crashes produced under `fuzz/artifacts/` are local investigation inputs. A
minimized, non-confidential reproducer must become a deterministic Rust or
Python regression test before a parser fix is considered complete.
