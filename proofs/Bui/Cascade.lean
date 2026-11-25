import Bui.Specificity

/-!
# Cascade order

`cascade_order` in `bindings/rust/blenderui/src/layout.rs` lists an element's declarations for
the two passes of `compute`, which apply the normal ones, then the `!important` ones, each in list
order, so a later declaration overrides an earlier one. This file proves the order it produces is
CSS Cascade 5's (§6.1): for any two declarations of the same importance, the one listed later is
the one the specification says wins.

What the Rust code emits, as a sort key (rules arrive sorted by `(layer, specificity, order)`,
unlayered ranking above every layer):

* normal: UA rules, then author rules (by layer, unlayered last), then the element's own;
* important: author rules (unlayered, then the layers from the last to the first), then the
  element's own, then UA rules.

The precedence below is the specification's, step by step: origin and importance, then
element-attached (the style attribute; BUI's own declarations on an element), then layers, then
specificity, then order of appearance.
-/


namespace Bui

inductive Origin where
  | ua
  | author
deriving DecidableEq

/-- One declaration that applies to an element. -/
structure Decl where
  origin : Origin
  /-- Element-attached (the style attribute; BUI's own declarations on the element). -/
  own : Bool
  /-- Cascade layer: `none` for unlayered, else its rank (earlier layers rank lower). -/
  layer : Option Nat
  spec : Spec
  /-- Order of appearance. -/
  order : Nat
  important : Bool

/-- What can occur: element-attached and UA declarations are unlayered, and only author
declarations are element-attached. Layer ranks stay below `bound` (the Rust code ranks unlayered
rules at `u16::MAX`, above every layer). -/
def Decl.wf (bound : Nat) (d : Decl) : Prop :=
  (d.own → d.origin = .author ∧ d.layer = none) ∧
  (d.origin = .ua → d.layer = none) ∧
  (∀ l, d.layer = some l → l < bound)

/-- Lexicographic `<` on equal-length keys. -/
def lexLt : List Nat → List Nat → Prop
  | [], [] => False
  | x :: xs, y :: ys => x < y ∨ (x = y ∧ lexLt xs ys)
  | _, _ => False

/-! ## The Rust order -/

/-- Where the Rust code lists a normal declaration. -/
def rustNormal (bound : Nat) (d : Decl) : List Nat :=
  [ match d.origin, d.own with
    | .ua, _ => 0
    | .author, false => 1
    | .author, true => 2
  , match d.layer with
    | none => bound
    | some l => l
  , d.spec.a, d.spec.b, d.spec.c, d.order ]

/-- Where the Rust code lists an important declaration. -/
def rustImportant (bound : Nat) (d : Decl) : List Nat :=
  [ match d.origin, d.own with
    | .author, false => 0
    | .author, true => 1
    | .ua, _ => 2
  , match d.layer with
    | none => 0
    | some l => bound - l
  , d.spec.a, d.spec.b, d.spec.c, d.order ]

/-! ## The specification's precedence -/

/-- Whether `y` wins over `x` by specificity then order of appearance. -/
def laterBySpecOrder (x y : Decl) : Prop :=
  x.spec.a < y.spec.a ∨ (x.spec.a = y.spec.a ∧ (x.spec.b < y.spec.b ∨ (x.spec.b = y.spec.b ∧
    (x.spec.c < y.spec.c ∨ (x.spec.c = y.spec.c ∧ x.order < y.order)))))

/-- CSS Cascade 5 §6.1: `y` wins over `x`, both normal. -/
def winsNormal (x y : Decl) : Prop :=
  -- origin: author over user agent
  (x.origin = .ua ∧ y.origin = .author) ∨
  (x.origin = y.origin ∧ (
    -- element-attached over rules
    (x.own = false ∧ y.own = true) ∨
    (x.own = y.own ∧ (
      -- layers: unlayered over layered, later layers over earlier
      (x.layer.isSome ∧ y.layer = none) ∨
      (∃ lx ly, x.layer = some lx ∧ y.layer = some ly ∧ lx < ly) ∨
      (x.layer = y.layer ∧ laterBySpecOrder x y)))))

/-- CSS Cascade 5 §6.1: `y` wins over `x`, both `!important`. -/
def winsImportant (x y : Decl) : Prop :=
  -- origin: important user agent over important author
  (x.origin = .author ∧ y.origin = .ua) ∨
  (x.origin = y.origin ∧ (
    -- element-attached over rules
    (x.own = false ∧ y.own = true) ∨
    (x.own = y.own ∧ (
      -- layers reverse: layered over unlayered, earlier layers over later
      (x.layer = none ∧ y.layer.isSome) ∨
      (∃ lx ly, x.layer = some lx ∧ y.layer = some ly ∧ ly < lx) ∨
      (x.layer = y.layer ∧ laterBySpecOrder x y)))))

/-! ## They agree -/

/-- Normal declarations: the Rust code lists `y` after `x` exactly when `y` wins. -/
theorem normal_order_is_the_cascade (bound : Nat) (x y : Decl) (hx : x.wf bound) (hy : y.wf bound) :
    lexLt (rustNormal bound x) (rustNormal bound y) ↔ winsNormal x y := by
  obtain ⟨xo, xs, xl, xsp, xord, _⟩ := x
  obtain ⟨yo, ys, yl, ysp, yord, _⟩ := y
  obtain ⟨xa, xb, xc⟩ := xsp
  obtain ⟨ya, yb, yc⟩ := ysp
  obtain ⟨hx1, hx2, hx3⟩ := hx
  obtain ⟨hy1, hy2, hy3⟩ := hy
  simp only at hx1 hx2 hx3 hy1 hy2 hy3
  cases xo <;> cases yo <;> cases xs <;> cases ys <;> cases xl <;> cases yl <;>
    simp_all [rustNormal, winsNormal, laterBySpecOrder, lexLt] <;>
    first
    | omega
    | (rename_i lx ly; have := hx3 lx rfl; have := hy3 ly rfl; omega)
    | (rename_i l; have := hx3 l rfl; omega)
    | (rename_i l; have := hy3 l rfl; omega)

/-- Important declarations: the Rust code lists `y` after `x` exactly when `y` wins. -/
theorem important_order_is_the_cascade (bound : Nat) (x y : Decl) (hx : x.wf bound) (hy : y.wf bound) :
    lexLt (rustImportant bound x) (rustImportant bound y) ↔ winsImportant x y := by
  obtain ⟨xo, xs, xl, xsp, xord, _⟩ := x
  obtain ⟨yo, ys, yl, ysp, yord, _⟩ := y
  obtain ⟨xa, xb, xc⟩ := xsp
  obtain ⟨ya, yb, yc⟩ := ysp
  obtain ⟨hx1, hx2, hx3⟩ := hx
  obtain ⟨hy1, hy2, hy3⟩ := hy
  simp only at hx1 hx2 hx3 hy1 hy2 hy3
  cases xo <;> cases yo <;> cases xs <;> cases ys <;> cases xl <;> cases yl <;>
    simp_all [rustImportant, winsImportant, laterBySpecOrder, lexLt] <;>
    first
    | omega
    | (rename_i lx ly; have := hx3 lx rfl; have := hy3 ly rfl; omega)
    | (rename_i l; have := hx3 l rfl; omega)
    | (rename_i l; have := hy3 l rfl; omega)

/-- The order before the fix listed important declarations like normal ones (layers in order,
unlayered last), so an unlayered `!important` beat a layered one: the specification says the
layered one wins. -/
theorem old_important_order_was_wrong :
    let layered : Decl := ⟨.author, false, some 0, ⟨1, 0, 0⟩, 0, true⟩
    let unlayered : Decl := ⟨.author, false, none, ⟨1, 0, 0⟩, 1, true⟩
    lexLt (rustNormal 1 layered) (rustNormal 1 unlayered) ∧ winsImportant unlayered layered := by
  simp [rustNormal, lexLt, winsImportant]

end Bui
