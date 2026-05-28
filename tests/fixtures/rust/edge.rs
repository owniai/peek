// edge.rs — boundary behaviors: function-body NOT extracted, trait required vs provided,
// inherent vs trait impl, proc macro, attribute in signature, const fn,
// generic signatures, multi-line flattening

// ── Function-body definitions NOT extracted ──
fn outer() {
    let _x = 1;
    fn inner_fn() {}
    struct InnerStruct {}
}

// ── Trait required (declaration) vs provided (definition) ──
trait Shape {
    fn area(&self) -> f64;
    fn name(&self) -> &'static str {
        "shape"
    }
}

// ── Inherent impl method vs non-operator trait impl ──
struct S {}
impl S {
    fn inherent_add(&self) -> i32 { 0 }
}
impl Clone for S {
    fn clone(&self) -> S { S {} }
}

// ── Proc macro classified as Macro, not Function ──
#[proc_macro]
pub fn make_answer(_input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    proc_macro::TokenStream::new()
}

// ── Attribute included in signature ──
#[inline]
fn inline_func() {}

// ── const fn is Function, not Const ──
const fn const_func() {}

// ── Generic struct ──
struct Wrapper<T> {
    inner: T,
}

// ── Generic function with where clause ──
fn generic_fn<T>(x: T) -> T where T: Clone {
    x.clone()
}

// ── Multi-line signature flattening ──
pub async fn complex_sig<'a, T: Clone + Send + 'static>(
    x: &'a [T],
    y: i32,
) -> Result<Vec<T>, Box<dyn std::error::Error>> {
    Ok(x.to_vec())
}

// ── ModuleDeclaration with attribute ──
#[cfg(test)]
mod tests_mod;

// ── Mutable static (static mut) is Var ──
static mut MUTABLE_GLOBAL: i32 = 1;