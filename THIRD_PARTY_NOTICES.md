# Licensing boundaries and third-party notices

## Earlier sldkit material

`LICENSES/sldkit-legacy-MIT.txt` preserves the original MIT notice, including its
`sldkit contributors` copyright line. It applies to material previously offered
under MIT, including public source at `c43b5ef752a0e58535b7f8f744f7d7c68e3c0e90`.
Those permissions survive inclusion in sldkit 0.2.0 and later. UnRobotics Inc.
is identified as licensor, not as the asserted owner of all earlier contributions.
New project material is offered under PolyForm Noncommercial 1.0.0 and separate
commercial agreements, subject to the exceptions below.

## Shared readers and bundled code

- `parasolid-core` 0.2.0, MIT AND Apache-2.0, from
  [parasolid-kit](https://github.com/monozukuri-ai/parasolid-kit).
  `LICENSES/parasolid-core-MIT.txt` preserves its MIT text. Adopted partial
  readers retain Apache-2.0; their provenance is reproduced in
  `LICENSES/parasolid-core-PARTIAL_READERS.md`. The Apache text is
  `LICENSES/Apache-2.0.txt`. The separate project retains its licenses.
- `cadmpeg-codec-sldprt` 0.5.3+sldkit.4 and its existing sldkit patches,
  `cadmpeg-container`, `cadmpeg-core` and `cadmpeg-ir` 0.5.3, Apache-2.0.
  Upstream: [cadmpeg v0.5.3](https://github.com/cadmpeg/cadmpeg/tree/v0.5.3),
  revision `dbb308e52ab05c911b26963a02fbb294eefab3ce`.
  Source headers, vendor LICENSE and `vendor/cadmpeg-codec-sldprt/PATCHES.md`
  preserve attribution and modifications. The vendor subtree stays Apache-2.0,
  including this migration's dependency update. The upstream root text is
  `LICENSES/Apache-2.0.txt`; registry container/core/ir packages omit a separate
  license-text file. Source headers remain in the supplied vendor sources.
- Three.js 0.180.0, including OrbitControls, MIT, from
  [Three.js r180](https://github.com/mrdoob/three.js/tree/r180).
  `LICENSES/three-MIT.txt` preserves its full notice, also bundled in viewer
  assets, JavaScript and standalone HTML. sldkit's own viewer follows the
  new-material/earlier-MIT boundary described above.
- Zstandard's C implementation, bundled through `zstd-sys`, is used under
  BSD-3-Clause. Its copyright and full terms are retained in the dependency
  catalog alongside the Rust wrapper's selected MIT terms.

## Locked dependency notices

`LICENSES/rust-dependencies.json` records registry packages from the main,
vendor and fuzz lockfiles, source checksums, upstream and selected licenses,
notice origins and hashes, and documented exclusions.
`LICENSES/rust-dependencies.txt` reproduces the corresponding notices.
The catalog includes build, development and target-specific packages for
attribution; it does not claim that all are linked into every wheel.
MIT is selected where offered as an alternative; mandatory Apache, Unicode,
BSD and exception terms are retained for their respective components.
The separate fuzz-only libFuzzer engine (NCSA) and UEFI-only r-efi are excluded
with their versions and reasons recorded. Neither is bundled in Python distributions.

The distribution SPDX expression covers new PolyForm material, earlier MIT,
bundled MIT JavaScript, Apache-2.0 readers and BSD-3-Clause zstd. Conditions
apply to corresponding portions, not as a choice of license for the entire
project. Build-tool notices do not by themselves make those tools part of the
runtime. Downstream OEMs must also honor the terms of any additional software
they bundle. Commercial agreements create no rights in third-party code,
data, trademarks or patents that the licensor cannot grant.

## References, samples and outputs

Public Microsoft specifications and independently observed file structures
inform the parser; see [parser provenance](docs/parser-provenance.md).
No proprietary SDK or SolidWorks installation is included. The project is
not affiliated with or endorsed by the owners of the SolidWorks or Parasolid names.
External CAD files, private fixtures and validation outputs are excluded from
Python distributions. Their rights are tracked separately in `corpus/`;
public download availability does not establish redistribution permission.
Extracted customer data retains its rights. Generated HTML also includes
viewer software, whose notices and terms accompany the output.
