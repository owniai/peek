//! Tree-sitter AST explorer for Swift: dumps S-expression with field names.

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
    let args: Vec<String> = std::env::args().collect();
    let source = if args.len() > 1 {
        std::fs::read_to_string(&args[1]).unwrap()
    } else {
        r#"
class MyClass {
    var name: String
    let id: Int
    func greet() -> String {
        return "Hello"
    }
    static func create() -> MyClass {
        return MyClass()
    }
}

struct Point {
    var x: Double
    var y: Double
    func distance() -> Double { return 0.0 }
}

enum Color {
    case red, green, blue
    func description() -> String { return "" }
}

protocol Drawable {
    func draw()
    var area: Double { get }
}

actor Counter {
    var value = 0
    func increment() -> Int {
        value += 1
        return value
    }
}

extension String {
    var trimmed: String { return self }
    func repeated(_ times: Int) -> String { return self }
}

typealias StringMap = [String: String]

let MAX_RETRIES = 3

func topLevelFunction(_ x: Int) -> Bool {
    return x > 0
}
"#
        .to_string()
    };

    println!("===== Swift AST Dump =====\n");

    let mut parser = tree_sitter::Parser::new();
    parser
        .set_language(&tree_sitter_swift::LANGUAGE.into())
        .expect("Swift language load failed");

    let tree = parser.parse(&source, None).unwrap();
    dump_sexp(tree.root_node(), &source, 0);
}
