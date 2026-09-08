# Third-party licenses

The following Rust crates are compiled into the modern Part geometry decoder:

- `parasolid-core` 0.1.0-dev6 — MIT AND Apache-2.0 (headers and shared partial readers)
- `cadmpeg-codec-sldprt` 0.5.3+sldkit.4 — Apache-2.0 (patched source)
- `cadmpeg-container` 0.5.3 — Apache-2.0
- `cadmpeg-core` 0.5.3 — Apache-2.0
- `cadmpeg-ir` 0.5.3 — Apache-2.0

Source: <https://github.com/cadmpeg/cadmpeg/tree/v0.5.3>

The codec patch restores a bounded native body hierarchy and configuration
membership profile. Its source and modification record are in
`vendor/cadmpeg-codec-sldprt/PATCHES.md` in the source distribution.

The Apache License 2.0 text is included as
[`Apache-2.0.txt`](Apache-2.0.txt). The upstream release does not publish a
`NOTICE` file. Other transitive dependencies are tracked by `Cargo.lock`; a
complete generated dependency-license inventory is part of release hardening.

`parasolid-core` is published from
<https://github.com/monozukuri-ai/parasolid-kit>. Its copyright and MIT license
are included in [`parasolid-core-MIT.txt`](parasolid-core-MIT.txt).

The adopted partial-reader files in parasolid-core remain Apache-2.0 licensed;
its `PARTIAL_READERS.md` records their origin and modifications. The Apache text
above covers those readers as well as the retained cadmpeg adapter.

The offline HTML viewer bundles Three.js 0.180.0 (MIT), including OrbitControls,
from <https://github.com/mrdoob/three.js/tree/r180>. Its license is in
[`three-MIT.txt`](three-MIT.txt) and is also embedded in generated HTML.
