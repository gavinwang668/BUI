# Proofs

Lean 4 models of parts of the Rust CSS engine (`bindings/rust/blenderui`), with proofs of the
properties the engine relies on. They cover the models, not the Rust code itself: the tests in
`bindings/rust/blenderui/tests/suite/css_spec.rs` check the code agrees with them.

| File | What it proves |
| --- | --- |
| `Bui/Specificity.lean` | Selector specificity (`spec_of` in `selector.rs`): a total order; `:where()` adds nothing; `:is()`, `:not()` and `:has()` count as their most specific argument; adding a simple selector never lowers it. `#eval vectors` prints the test cases `css_spec.rs` checks against the parser. |
| `Bui/Cascade.lean` | The order `cascade_order` in `layout.rs` lists declarations in is CSS Cascade 5's, for normal and `!important` declarations (origins, element-attached, layers, specificity, order); and the order before the fix was not. |

Build with `lake build` in this folder (Lean 4.36, no dependencies). `#print axioms` on each
theorem lists only Lean's standard axioms.
