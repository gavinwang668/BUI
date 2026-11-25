/-!
# Selector specificity

A model of `spec_of` in `bindings/rust/blenderui/src/selector.rs`, after Selectors 4 §17:
an id counts (1, 0, 0); a class, attribute or pseudo-class (0, 1, 0); a type or pseudo-element
(0, 0, 1); `:where()` nothing; `:is()`, `:not()` and `:has()` their most specific argument;
`:nth-child(An+B of S)` a pseudo-class plus its most specific argument.

The theorems pin down the properties the cascade relies on: the order is total, `:where()` adds
nothing, `:has()` counts like `:is()` (the Rust code counted it as one class until this was
checked), a function counts at least as much as any of its arguments, and adding a simple selector
never lowers a selector's specificity. `vectors` prints test cases for
`bindings/rust/blenderui/tests/suite/css_spec.rs`, which checks the Rust parser agrees.
-/

namespace Bui

/-- Specificity (a, b, c), compared lexicographically. -/
structure Spec where
  a : Nat
  b : Nat
  c : Nat
deriving DecidableEq, Repr

namespace Spec

def zero : Spec := ⟨0, 0, 0⟩

def add (x y : Spec) : Spec := ⟨x.a + y.a, x.b + y.b, x.c + y.c⟩

/-- Lexicographic `≤`. -/
def le (x y : Spec) : Prop :=
  x.a < y.a ∨ (x.a = y.a ∧ (x.b < y.b ∨ (x.b = y.b ∧ x.c ≤ y.c)))

instance (x y : Spec) : Decidable (le x y) := by unfold le; exact inferInstance

def max (x y : Spec) : Spec := if le x y then y else x

theorem le_refl (x : Spec) : le x x := by
  unfold le; omega

theorem le_trans {x y z : Spec} : le x y → le y z → le x z := by
  unfold le; omega

theorem le_total (x y : Spec) : le x y ∨ le y x := by
  unfold le; omega

theorem le_antisymm {x y : Spec} : le x y → le y x → x = y := by
  rcases x with ⟨a, b, c⟩; rcases y with ⟨a', b', c'⟩
  unfold le; simp only [mk.injEq]; omega

theorem le_max_left (x y : Spec) : le x (max x y) := by
  unfold max; split
  · assumption
  · exact le_refl x

theorem le_max_right (x y : Spec) : le y (max x y) := by
  unfold max; split
  · exact le_refl y
  · rcases le_total x y with h | h
    · contradiction
    · exact h

/-- Adding anything never lowers a specificity. -/
theorem le_add_left (x y : Spec) : le y (add x y) := by
  unfold le add; simp only; omega

end Spec

mutual
  /-- A simple selector. Arguments of functional pseudo-classes are selector lists. -/
  inductive Simple where
    | universal
    | tag (name : String)
    | id (name : String)
    | cls (name : String)
    | attr (name : String)
    | pseudoClass (name : String)
    | is (args : List Sel)
    | not (args : List Sel)
    | where_ (args : List Sel)
    | has (args : List Sel)
    | nth (of : Option (List Sel))

  /-- A complex selector: compounds (descendant combinators between them), and whether it ends
  in a pseudo-element. -/
  inductive Sel where
    | mk (compounds : List (List Simple)) (pseudoElement : Bool)
end

mutual
  def Simple.spec : Simple → Spec
    | .universal => Spec.zero
    | .tag _ => ⟨0, 0, 1⟩
    | .id _ => ⟨1, 0, 0⟩
    | .cls _ | .attr _ | .pseudoClass _ => ⟨0, 1, 0⟩
    | .is l | .not l | .has l => maxSpec l
    | .where_ _ => Spec.zero
    | .nth none => ⟨0, 1, 0⟩
    | .nth (some l) => Spec.add ⟨0, 1, 0⟩ (maxSpec l)

  /-- The most specific selector of a list (`(0, 0, 0)` for none). -/
  def maxSpec : List Sel → Spec
    | [] => Spec.zero
    | s :: rest => Spec.max (Sel.spec s) (maxSpec rest)

  def compoundSpec : List Simple → Spec
    | [] => Spec.zero
    | s :: rest => Spec.add (Simple.spec s) (compoundSpec rest)

  def compoundsSpec : List (List Simple) → Spec
    | [] => Spec.zero
    | c :: rest => Spec.add (compoundSpec c) (compoundsSpec rest)

  def Sel.spec : Sel → Spec
    | .mk cs pe => Spec.add (compoundsSpec cs) (if pe then ⟨0, 0, 1⟩ else Spec.zero)
end

/-- `:where()` adds nothing. -/
theorem where_adds_nothing (l : List Sel) : (Simple.where_ l).spec = Spec.zero := by
  simp [Simple.spec]

/-- `:has()` counts exactly like `:is()` over the same arguments. -/
theorem has_counts_like_is (l : List Sel) : (Simple.has l).spec = (Simple.is l).spec := by
  simp [Simple.spec]

/-- `:not()` counts exactly like `:is()` over the same arguments. -/
theorem not_counts_like_is (l : List Sel) : (Simple.not l).spec = (Simple.is l).spec := by
  simp [Simple.spec]

/-- The list maximum is at least each member's specificity. -/
theorem maxSpec_ge {s : Sel} : ∀ {l : List Sel}, s ∈ l → Spec.le s.spec (maxSpec l)
  | _ :: rest, .head _ => by simp only [maxSpec]; exact Spec.le_max_left _ _
  | t :: rest, .tail _ h => by
      simp only [maxSpec]
      exact Spec.le_trans (maxSpec_ge h) (Spec.le_max_right _ _)

/-- `:is()` (and so `:not()` / `:has()`) counts at least as much as any argument. -/
theorem is_ge_argument {s : Sel} {l : List Sel} (h : s ∈ l) : Spec.le s.spec (Simple.is l).spec := by
  simp only [Simple.spec]; exact maxSpec_ge h

/-- Adding a simple selector to a compound never lowers its specificity. -/
theorem compound_monotone (s : Simple) (c : List Simple) :
    Spec.le (compoundSpec c) (compoundSpec (s :: c)) := by
  simp only [compoundSpec]; exact Spec.le_add_left _ _

/-- Adding a compound to a selector never lowers its specificity. -/
theorem compounds_monotone (c : List Simple) (cs : List (List Simple)) :
    Spec.le (compoundsSpec cs) (compoundsSpec (c :: cs)) := by
  simp only [compoundsSpec]; exact Spec.le_add_left _ _

/-! ## Test vectors

Selectors written in CSS with the specificity the model gives; `#eval vectors` prints them as
Rust tuples for `css_spec.rs`. -/

mutual
  def Simple.css : Simple → String
    | .universal => "*"
    | .tag n => n
    | .id n => "#" ++ n
    | .cls n => "." ++ n
    | .attr n => "[" ++ n ++ "]"
    | .pseudoClass n => ":" ++ n
    | .is l => ":is(" ++ listCss l ++ ")"
    | .not l => ":not(" ++ listCss l ++ ")"
    | .where_ l => ":where(" ++ listCss l ++ ")"
    | .has l => ":has(" ++ listCss l ++ ")"
    | .nth none => ":nth-child(2n+1)"
    | .nth (some l) => ":nth-child(2n+1 of " ++ listCss l ++ ")"

  def listCss : List Sel → String
    | [] => ""
    | [s] => Sel.css s
    | s :: rest => Sel.css s ++ ", " ++ listCss rest

  def compoundCss : List Simple → String
    | [] => ""
    | s :: rest => Simple.css s ++ compoundCss rest

  def compoundsCss : List (List Simple) → String
    | [] => ""
    | [c] => compoundCss c
    | c :: rest => compoundCss c ++ " " ++ compoundsCss rest

  def Sel.css : Sel → String
    | .mk cs pe => compoundsCss cs ++ (if pe then "::before" else "")
end

open Simple in
/-- Selectors that exercise every rule of the model. -/
def cases : List Sel :=
  let one (c : List Simple) := Sel.mk [c] false
  [ one [tag "div"]
  , one [tag "p", id "a", cls "b"]
  , Sel.mk [[tag "ul"], [tag "li", cls "x"], [pseudoClass "hover"]] false
  , one [cls "p", has [one [id "d"]]]
  , one [has [one [cls "x"], one [id "y", cls "z"]]]
  , one [is [one [tag "p"], one [id "q"]]]
  , one [not [one [cls "a"], one [cls "b", cls "c"]]]
  , one [where_ [one [id "w"]], cls "k"]
  , one [nth none]
  , one [nth (some [one [cls "x"], one [id "y"]])]
  , Sel.mk [[tag "p"]] true
  , one [universal, attr "data-x"]
  , one [is [one [has [one [id "deep"]]]]]
  ]

def vectors : String :=
  String.intercalate "\n" (cases.map fun s =>
    let p := s.spec
    s!"(\"{s.css}\", ({p.a}, {p.b}, {p.c})),")

#eval IO.println vectors

end Bui
