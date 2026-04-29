//! Check field names for macro_definition nodes.

fn main() {
    let mut parser = tree_sitter::Parser::new();
    parser
        .set_language(&tree_sitter_rust::LANGUAGE.into())
        .unwrap();

    // Test 1: basic macro_rules!
    let src = r#"macro_rules! say_hello {
    () => { println!("Hello") };
}"#;
    let tree = parser.parse(src, None).unwrap();
    let root = tree.root_node();
    let macro_def = root.child(0).unwrap();

    println!("=== basic macro_rules! ===");
    println!("node kind: {}", macro_def.kind());
    println!("child count: {}", macro_def.child_count());

    // Check if child_by_field_name("name") works
    if let Some(name_node) = macro_def.child_by_field_name("name") {
        println!(
            "child_by_field_name(\"name\"): kind={}, text=\"{}\"",
            name_node.kind(),
            name_node.utf8_text(src.as_bytes()).unwrap()
        );
    } else {
        println!("child_by_field_name(\"name\"): None");
    }

    // Print all children with their field names
    let mut cursor = macro_def.walk();
    for (i, child) in macro_def.children(&mut cursor).enumerate() {
        let fname = macro_def.field_name_for_child(i as u32);
        println!("  child[{}]: kind={}, field={:?}", i, child.kind(), fname);
    }

    // Test 2: macro_rules! inside impl
    let src2 = "impl Foo {\n    macro_rules! inner {\n        () => {};\n    }\n}";
    let tree2 = parser.parse(src2, None).unwrap();
    let impl_node = tree2.root_node().child(0).unwrap();
    let decl_list = impl_node.child_by_field_name("body").unwrap();
    let macro_def2 = decl_list.child(1).unwrap(); // skip '{'

    println!("\n=== macro_rules! inside impl ===");
    println!("node kind: {}", macro_def2.kind());
    if let Some(name_node) = macro_def2.child_by_field_name("name") {
        println!(
            "child_by_field_name(\"name\"): kind={}, text=\"{}\"",
            name_node.kind(),
            name_node.utf8_text(src2.as_bytes()).unwrap()
        );
    } else {
        println!("child_by_field_name(\"name\"): None");
    }

    // Test 3: with attribute
    let src3 = "#[macro_export]\nmacro_rules! exported {\n    () => {};\n}";
    let tree3 = parser.parse(src3, None).unwrap();
    let root3 = tree3.root_node();

    println!("\n=== with #[macro_export] ===");
    println!("root child count: {}", root3.child_count());
    for (i, child) in root3.children(&mut root3.walk()).enumerate() {
        let fname = root3.field_name_for_child(i as u32);
        println!(
            "  root child[{}]: kind={}, field={:?}",
            i,
            child.kind(),
            fname
        );
    }

    // The macro_definition should be child(1) or later
    for i in 0..root3.child_count() {
        let child = root3.child(i as u32).unwrap();
        if child.kind() == "macro_definition" {
            if let Some(name_node) = child.child_by_field_name("name") {
                println!(
                    "macro_def child_by_field_name(\"name\"): kind={}, text=\"{}\"",
                    name_node.kind(),
                    name_node.utf8_text(src3.as_bytes()).unwrap()
                );
            } else {
                println!("macro_def child_by_field_name(\"name\"): None");
            }
            break;
        }
    }

    // Test 4: macro_rules! inside mod
    let src4 = "mod m {\n    macro_rules! nested {\n        () => {};\n    }\n}";
    let tree4 = parser.parse(src4, None).unwrap();
    println!("\n=== macro_rules! inside mod ===");
    let mod_node = tree4.root_node().child(0).unwrap();
    let mod_body = mod_node.child_by_field_name("body").unwrap();
    let macro_def4 = mod_body.child(1).unwrap();
    println!("kind: {}", macro_def4.kind());
    if let Some(name_node) = macro_def4.child_by_field_name("name") {
        println!(
            "child_by_field_name(\"name\"): {}",
            name_node.utf8_text(src4.as_bytes()).unwrap()
        );
    } else {
        println!("child_by_field_name(\"name\"): None");
    }
}
