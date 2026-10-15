# Local patches to edtui 0.11.7

Vendored from crates.io (MIT, see LICENSE.md) and used via
`[patch.crates-io]` in the workspace `Cargo.toml`.

- **Word-aware line wrapping** (`src/view/line_wrapper.rs`,
  `src/state/view.rs`): wrapped rows break after the last space that fits
  instead of mid-word; words longer than a row are still split. One
  function (`split_lengths`) decides the breaks for rendering, viewport
  scrolling and mouse mapping, and keeps every character, so cursor
  positions are unchanged. Candidate for an upstream pull request.
- Removed `assets/`, `resources/`, `examples/` to keep the vendored copy small.
