mod common;

use common::{parse_defs, peek};

#[test]
fn peek_for_finds_csharp_class_scope() {
    let output = peek(&["-w", "-k", "class", "User", "tests/fixtures/csharp"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].kind, "class");
    assert_eq!(results[0].scope, "MyApp.Models.User");
}

#[test]
fn peek_for_csharp_nested_class_scope() {
    let output = peek(&["-k", "class", "Inner", "tests/fixtures/csharp"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].scope, "MyApp.Models.Container.Inner");
}

#[test]
fn peek_for_csharp_file_scoped_namespace() {
    let output = peek(&["-k", "class", "UserService", "tests/fixtures/csharp"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].scope, "MyApp.Services.UserService");
}

#[test]
fn peek_for_csharp_delegate_scope() {
    let output = peek(&["-w", "-k", "delegate", "Validator", "tests/fixtures/csharp"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].scope, "MyApp.Models.User.Validator");
}

#[test]
fn peek_for_csharp_event_scope() {
    let output = peek(&["-k", "event", "OnNameChanged", "tests/fixtures/csharp"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].scope, "MyApp.Models.User.OnNameChanged");
}

#[test]
fn peek_for_csharp_const_scope() {
    let output = peek(&["-k", "const", "MaxAge", "tests/fixtures/csharp"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].scope, "MyApp.Models.User.MaxAge");
}

#[test]
fn peek_for_csharp_method_scope() {
    let output = peek(&["-k", "method", "GetName", "tests/fixtures/csharp"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].scope, "MyApp.Models.User.GetName");
}

// === Comprehensive fixture tests ===

#[test]
fn peek_for_csharp_comprehensive_file_scoped_interface() {
    let output = peek(&["-k", "interface", "IEmailSender", "tests/fixtures/csharp"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(
        results
            .iter()
            .any(|r| r.scope == "Comprehensive.Services.IEmailSender" && r.kind == "interface")
    );
}

#[test]
fn peek_for_csharp_comprehensive_file_scoped_enum() {
    let output = peek(&["-k", "enum", "EmailPriority", "tests/fixtures/csharp"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(
        results
            .iter()
            .any(|r| r.scope == "Comprehensive.Services.EmailPriority" && r.kind == "enum")
    );
}

#[test]
fn peek_for_csharp_comprehensive_file_scoped_delegate() {
    let output = peek(&["-k", "delegate", "EmailValidator", "tests/fixtures/csharp"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(
        results
            .iter()
            .any(|r| r.scope == "Comprehensive.Services.EmailValidator" && r.kind == "delegate")
    );
}

#[test]
fn peek_for_csharp_comprehensive_record_variants() {
    // record
    let output = peek(&["-k", "record", "Mailbox", "tests/fixtures/csharp"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    assert!(stdout.contains("Mailbox"));

    // record struct
    let output = peek(&["-k", "record", "AddressPair", "tests/fixtures/csharp"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    assert!(stdout.contains("AddressPair"));

    // record class
    let output = peek(&["-k", "record", "EmailTemplate", "tests/fixtures/csharp"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    assert!(stdout.contains("EmailTemplate"));
}

#[test]
fn peek_for_csharp_comprehensive_multi_const() {
    // SmtpPort from multi-variable const: const int SmtpPort = 587, ImapPort = 993;
    let output = peek(&["-k", "const", "SmtpPort", "tests/fixtures/csharp"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(
        results
            .iter()
            .any(|r| r.scope == "Comprehensive.Services.EmailConfig.SmtpPort" && r.kind == "const")
    );

    let output = peek(&["-k", "const", "ImapPort", "tests/fixtures/csharp"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(
        results
            .iter()
            .any(|r| r.scope == "Comprehensive.Services.EmailConfig.ImapPort" && r.kind == "const")
    );
}

#[test]
fn peek_for_csharp_comprehensive_events() {
    let output = peek(&["-k", "event", "OnEmailSent", "tests/fixtures/csharp"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(
        results.iter().any(
            |r| r.scope == "Comprehensive.Services.EmailEvents.OnEmailSent" && r.kind == "event"
        )
    );

    let output = peek(&["-k", "event", "OnEmailFailed", "tests/fixtures/csharp"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(results.iter().any(
        |r| r.scope == "Comprehensive.Services.EmailEvents.OnEmailFailed" && r.kind == "event"
    ));
}

#[test]
fn peek_for_csharp_comprehensive_file_scoped_struct() {
    let output = peek(&["-k", "struct", "EmailMessage", "tests/fixtures/csharp"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(
        results
            .iter()
            .any(|r| r.scope == "Comprehensive.Services.EmailMessage" && r.kind == "struct")
    );
}

#[test]
fn peek_for_csharp_comprehensive_file_scoped_const() {
    let output = peek(&["-k", "const", "MaxRetries", "tests/fixtures/csharp"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(
        results.iter().any(
            |r| r.scope == "Comprehensive.Services.EmailConfig.MaxRetries" && r.kind == "const"
        )
    );
}

#[test]
fn peek_for_csharp_no_false_positive() {
    // "using" directives should never be extracted
    let output = peek(&["System", "tests/fixtures/csharp"]);
    assert_eq!(
        output.status.code(),
        Some(1),
        "expected no match for using directive"
    );
}

// --- Supplementary: kind filter and constructor validation ---

#[test]
fn peek_for_csharp_kind_filter_excludes_mismatch() {
    // "User" is a class, searching with --kind struct should find nothing
    let output = peek(&["-k", "struct", "User", "tests/fixtures/csharp"]);
    assert_eq!(
        output.status.code(),
        Some(1),
        "expected no match for 'User' with --kind struct"
    );
}

#[test]
fn peek_for_csharp_static_factory_method_scope() {
    // C# constructors are named after the class; the static factory method
    // "Create" verifies method scope behavior.
    let output = peek(&["-k", "method", "Create", "tests/fixtures/csharp"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(
        results
            .iter()
            .any(|r| r.scope == "MyApp.Models.User.Create" && r.kind == "method")
    );
}

// --- Property accessor (Getter/Setter) tests ---

#[test]
fn peek_for_csharp_property_getter() {
    // sample.cs has: public string Name { get; set; }
    let output = peek(&["-k", "getter", "Name", "tests/fixtures/csharp"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(
        results
            .iter()
            .any(|r| r.scope == "MyApp.Models.User.Name" && r.kind == "getter"),
        "expected getter for User.Name, got: {results:?}"
    );
}

#[test]
fn peek_for_csharp_property_setter() {
    let output = peek(&["-k", "setter", "Name", "tests/fixtures/csharp"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(
        results
            .iter()
            .any(|r| r.scope == "MyApp.Models.User.Name" && r.kind == "setter"),
        "expected setter for User.Name, got: {results:?}"
    );
}

#[test]
fn peek_for_csharp_readonly_property_getter() {
    // comprehensive.cs has: public string To { get; } (read-only property in EmailMessage struct)
    let output = peek(&["-k", "getter", "To", "tests/fixtures/csharp"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(
        results.iter().any(|r| r.kind == "getter"),
        "expected getter for 'To', got: {results:?}"
    );
}

// --- Field tests ---

#[test]
fn peek_for_csharp_field_in_class() {
    // Product.Id is a plain field (no { get; set; })
    let output = peek(&["-k", "field", "Id", "tests/fixtures/csharp"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(
        results
            .iter()
            .any(|r| r.scope == "Comprehensive.Services.Product.Id" && r.kind == "field"),
        "expected field Product.Id, got: {results:?}"
    );
}

#[test]
fn peek_for_csharp_field_in_struct() {
    // Coordinate.Latitude is a struct field
    let output = peek(&["-k", "field", "Latitude", "tests/fixtures/csharp"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(
        results
            .iter()
            .any(|r| r.scope == "Comprehensive.Services.Coordinate.Latitude" && r.kind == "field"),
        "expected field Coordinate.Latitude, got: {results:?}"
    );
}

#[test]
fn peek_for_csharp_field_not_const() {
    // _name is a private field, not a const
    let output = peek(&["-k", "field", "_name", "tests/fixtures/csharp"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(
        results
            .iter()
            .any(|r| r.scope == "Comprehensive.Services.Product._name" && r.kind == "field"),
        "expected field Product._name, got: {results:?}"
    );
    // Should not appear as const
    assert!(
        !results
            .iter()
            .any(|r| r.kind == "const" && r.scope.contains("_name"))
    );
}

#[test]
fn peek_for_csharp_property_kind() {
    // DisplayName is a property (has { get; set; })
    let output = peek(&["-k", "property", "DisplayName", "tests/fixtures/csharp"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(
        results.iter().any(
            |r| r.scope == "Comprehensive.Services.Product.DisplayName" && r.kind == "property"
        ),
        "expected property Product.DisplayName, got: {results:?}"
    );
}

#[test]
fn peek_for_csharp_readonly_property() {
    // Category has only getter: public string Category { get; }
    let output = peek(&["-k", "property", "Category", "tests/fixtures/csharp"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(
        results
            .iter()
            .any(|r| r.scope == "Comprehensive.Services.Product.Category" && r.kind == "property"),
        "expected property Product.Category, got: {results:?}"
    );
}

#[test]
fn peek_for_csharp_field_in_nested_sample() {
    // sample.cs Point struct has: public double X; public double Y;
    let output = peek(&["-k", "field", "X", "tests/fixtures/csharp"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(
        results
            .iter()
            .any(|r| r.scope == "MyApp.Models.Point.X" && r.kind == "field"),
        "expected field Point.X, got: {results:?}"
    );
}

#[test]
fn peek_for_csharp_value_category_includes_field_property() {
    // -k value should expand to include field and property
    let output = peek(&["-k", "value", "Latitude", "tests/fixtures/csharp"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(
        results
            .iter()
            .any(|r| r.scope == "Comprehensive.Services.Coordinate.Latitude"),
        "expected Latitude in value category, got: {results:?}"
    );
}

// === Namespace tests ===

#[test]
fn peek_for_csharp_namespace_kind_braced() {
    let output = peek(&[
        "-k",
        "namespace",
        "-w",
        "MyApp.Models",
        "tests/fixtures/csharp",
    ]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(
        results
            .iter()
            .any(|r| r.scope == "MyApp.Models" && r.kind == "namespace"),
        "expected namespace MyApp.Models, got: {results:?}"
    );
}

#[test]
fn peek_for_csharp_namespace_kind_file_scoped() {
    let output = peek(&[
        "-k",
        "namespace",
        "-w",
        "MyApp.Services",
        "tests/fixtures/csharp",
    ]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(
        results
            .iter()
            .any(|r| r.scope == "MyApp.Services" && r.kind == "namespace"),
        "expected namespace MyApp.Services, got: {results:?}"
    );
}

#[test]
fn peek_for_csharp_namespace_kind_comprehensive() {
    let output = peek(&[
        "-k",
        "namespace",
        "-w",
        "Comprehensive.Services",
        "tests/fixtures/csharp",
    ]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(
        results
            .iter()
            .any(|r| r.scope == "Comprehensive.Services" && r.kind == "namespace"),
        "expected namespace Comprehensive.Services, got: {results:?}"
    );
}
