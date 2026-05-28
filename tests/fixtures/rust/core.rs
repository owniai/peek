// core.rs — all kind classifications, scope paths (::), signature formats, nested scope
// Each construct appears once; no duplicate coverage across core and edge.

use std::ops::{Add, Index};

// ── Function ──
fn top_func(x: i32) -> bool {
    true
}

// ── Const ──
const MAX_LEN: usize = 1024;

// ── Var (static) ──
static GLOBAL_COUNT: usize = 0;

// ── Macro ──
macro_rules! say_hello {
    () => { println!("hello") };
}

// ── Alias (type alias) ──
type Point = (f64, f64);

// ── Struct ──
struct MyStruct {
    // ── Field ──
    value: i32,
}

impl MyStruct {
    // ── Method ──
    fn get_value(&self) -> i32 {
        self.value
    }
    // ── Async Method (signature retains async) ──
    pub async fn async_method(&self) -> i32 { 0 }
}

// ── impl Display ──
impl std::fmt::Display for MyStruct {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.value)
    }
}

// ── Enum ──
enum Color {
    // ── Variant ──
    Red,
    Blue,
    Rgb { r: u8, g: u8, b: u8 },
}

// ── Union ──
union IntOrFloat {
    i: i32,
    f: f32,
}

// ── Trait ──
trait MyTrait {
    // ── AssociatedType ──
    type Item;

    fn required_method(&self) -> i32;
}

// ── Module (with body) ──
mod my_mod {
    fn mod_func() {}
}

// ── ModuleDeclaration (bodyless) ──
mod external_mod;

// ── Operator (trait impl) ──
struct Vec2 {
    x: f64,
    y: f64,
}

impl std::ops::Add for Vec2 {
    type Output = Self;
    fn add(self, rhs: Self) -> Self::Output {
        Self { x: self.x + rhs.x, y: self.y + rhs.y }
    }
}

// ── Subscript (Index trait impl) ──
struct MyVec {
    data: Vec<i32>,
}

impl std::ops::Index<usize> for MyVec {
    type Output = i32;
    fn index(&self, idx: usize) -> &i32 {
        &self.data[idx]
    }
}

// ── Destructor (Drop trait impl) ──
struct Resource {
    handle: i32,
}

impl Drop for Resource {
    fn drop(&mut self) {}
}

// ── Deep nested scope ──
mod outer {
    mod inner {
        fn deep_func() {}
    }
}