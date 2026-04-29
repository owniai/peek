// ============================================================
// Rust tree-sitter AST 实验样本
// 覆盖：fn（各种修饰符）、struct（各种形式）、属性（#[...]）、
//        impl（固有/trait）、泛型、mod 嵌套、trait 定义
// ============================================================

// === 1. 简单函数 ===
fn simple_func() {}

fn with_params(x: i32, y: &str) -> bool {
    true
}

// === 2. 可见性修饰 ===
pub fn pub_func() {}
pub(crate) fn pub_crate_func() {}
pub(super) fn pub_super_func() {}

// === 3. 函数修饰符 ===
async fn async_func() {}
unsafe fn unsafe_func() {}
const fn const_func() {}
extern "C" fn extern_func() {}

// === 4. 泛型函数 ===
fn generic_func<T>(x: T) -> T {
    x
}
fn generic_where<T>(x: T) -> T
where
    T: Clone,
{
    x.clone()
}
fn multi_generic<T, U>(x: T, y: U) -> (T, U) {
    (x, y)
}

// === 5. 属性标注函数（装饰器等价）===
#[inline]
fn inline_func() {}

#[test]
fn test_func() {}

#[cfg(test)]
fn cfg_func() {}

#[allow(unused)]
fn allow_func() {}

#[tokio::main]
async fn tokio_main() {}

// 多属性叠加
#[inline]
#[allow(dead_code)]
fn multi_attr_func() {}

// 属性带参数
#[cfg(all(test, feature = "full"))]
fn cfg_all_func() {}

// === 6. 简单 struct ===
struct UnitStruct;

struct TupleStruct(i32, String);

struct FieldStruct {
    x: i32,
    y: String,
}

// === 7. struct 可见性 ===
pub struct PubStruct {
    value: i32,
}

pub(crate) struct PubCrateStruct {
    value: i32,
}

// === 8. 泛型 struct ===
struct GenericStruct<T> {
    value: T,
}

struct BoundedStruct<T: Clone> {
    value: T,
}

struct LifetimeStruct<'a> {
    data: &'a str,
}

struct MultiGeneric<'a, T: Clone + 'a> {
    inner: &'a [T],
}

// === 9. 属性标注 struct ===
#[derive(Debug, Clone)]
struct DeriveStruct {
    x: i32,
}

#[repr(C)]
struct ReprStruct {
    x: i32,
}

#[derive(Debug)]
#[repr(transparent)]
struct MultiAttrStruct {
    inner: i32,
}

// === 10. 固有 impl ===
struct MyStruct {
    value: i32,
}

impl MyStruct {
    fn new(value: i32) -> Self {
        Self { value }
    }

    pub fn get_value(&self) -> i32 {
        self.value
    }

    pub async fn async_method(&self) -> i32 {
        self.value
    }

    #[inline]
    fn tagged_method(&self) -> i32 {
        self.value
    }
}

// === 11. 泛型 impl ===
impl<T> GenericStruct<T> {
    fn new(value: T) -> Self {
        Self { value }
    }

    fn into_value(self) -> T {
        self.value
    }
}

// === 12. trait impl ===
impl std::fmt::Display for MyStruct {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.value)
    }
}

// === 13. trait 定义 ===
trait MyTrait {
    fn required_method(&self) -> i32;

    fn provided_method(&self) -> i32 {
        42
    }
}

// === 14. mod 嵌套 ===
mod my_module {
    fn mod_func() {}

    pub fn pub_mod_func() {}

    struct ModStruct {}

    impl ModStruct {
        fn method() {}
    }

    mod nested {
        fn deep_func() {}
    }
}

// === 15. 复杂嵌套：mod + impl + 泛型 ===
mod container {
    struct Container<T> {
        inner: T,
    }

    impl<T> Container<T> {
        fn new(inner: T) -> Self {
            Self { inner }
        }
    }
}

// === 16. 复杂签名函数 ===
pub async fn complex_func<'a, T: Clone + Send + 'static>(
    x: &'a [T],
    y: i32,
) -> Result<Vec<T>, Box<dyn std::error::Error>> {
    Ok(x.to_vec())
}
