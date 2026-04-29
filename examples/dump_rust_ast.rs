//! Tree-sitter AST explorer for Rust: dumps S-expression with field names.

fn dump_sexp(node: tree_sitter::Node, source: &str, indent: usize) {
    let pad = "  ".repeat(indent);

    if !node.is_named() {
        let text: String = node
            .utf8_text(source.as_bytes())
            .unwrap_or("")
            .chars()
            .take(60)
            .collect();
        println!("{pad}\"{}\"", text.replace('\n', "\\n"));
        return;
    }

    let kind = node.kind();

    let children: Vec<_> = {
        let mut cursor = node.walk();
        node.children(&mut cursor).enumerate().collect()
    };

    if children.is_empty() {
        let text: String = node
            .utf8_text(source.as_bytes())
            .unwrap_or("")
            .chars()
            .take(60)
            .collect();
        println!("{pad}({kind} \"{text}\")");
        return;
    }

    println!("{pad}({kind}");

    for (i, child) in &children {
        let field_name = node.field_name_for_child(*i as u32);
        if let Some(fname) = field_name {
            println!("{pad}  {fname}:");
            dump_sexp(*child, source, indent + 3);
        } else {
            dump_sexp(*child, source, indent + 2);
        }
    }

    println!("{pad})");
}

fn main() {
    let path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "tests/fixtures/rust/sample.rs".to_string());

    println!("===== Rust AST: {} =====\n", path);

    let source = std::fs::read_to_string(&path).unwrap();
    let mut parser = tree_sitter::Parser::new();
    parser
        .set_language(&tree_sitter_rust::LANGUAGE.into())
        .unwrap();

    let tree = parser.parse(&source, None).unwrap();
    dump_sexp(tree.root_node(), &source, 0);
}
