# Paulino Gin Cryostat sample-stage CAD audit

## Decision

The six native CAD artifacts and three corresponding neutral exports listed
below at revision
`77751f2461a130f7f5d9ed4111da787ecb9cc4f2` are approved for parser testing and
redistribution under MIT, provided that the upstream copyright and license
notice accompanies any redistributed copy. `sldkit` does not vendor the files;
the decision allows an audited public corpus without adding CAD binaries to the
package or repository.

## Scope and evidence

- Source: <https://github.com/paulggin/solidworks-cryostat-sample-stage>
- License: the pinned repository's root `LICENSE` is MIT and names
  `Copyright (c) 2026 Paul Gin`.
- Authorship: the README names Paulino Gin as author and describes the native
  Part, Assembly, Drawing, and Simulation work as this project's deliverable.
- Inventory: the pinned `INVENTORY.md` identifies each selected native CAD file
  and its role. No selected file is identified as a supplier or GrabCAD model.
- The `CPW_Chip.SLDPRT` file is described as a stand-in for the author's
  companion CPW design, rather than an imported supplier model.

Selected paths:

- `cad/CPW_Chip.SLDPRT`
- `cad/sample_stage.SLDPRT`
- `cad/cable_bracket.SLDPRT`
- `cad/ChipStage.SLDASM`
- `cad/sample_stage.SLDDRW`
- `cad/cable_bracket.SLDDRW`
- `step/sample_stage.STEP`
- `step/cpw_chip.STEP`
- `step/cable_bracket.STEP`

The pinned `INVENTORY.md` labels the three STEP files as neutral exports, and
the README lists them alongside the native deliverables. The native and neutral
files were added in the same pinned commit. This supports artifact pairing; it
does not prove that a neutral export preserves native face or edge partitioning.

This is a repository-evidence audit, not legal advice or an independent
copyright-registration check.

## Reference-closure limitation

The Assembly stores `CPW_Stage.SLDPRT`, `Coax_BracketBottom.SLDPRT`, and
`Coax_Bracket.SLDPRT`, while the repository uses `sample_stage.SLDPRT` and
`cable_bracket.SLDPRT`. The Drawings similarly store `CPW_Stage.sldprt` and
`Coax_Bracket.sldprt`. The README declares the intended relationships, but the
literal saved-path closure is not present. Therefore this source satisfies the
independent audited three-document gate, but it is not counted as the controlled
Pack and Go gate.
