mod common;

use common::{parse_defs, peek};

#[test]
fn peek_for_finds_csharp_class_scope() {
    let output = peek(&["-k", "class", "User", "tests/fixtures/csharp"]);
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
    let output = peek(&["-k", "delegate", "Validator", "tests/fixtures/csharp"]);
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
    let output = peek(&["-k", "function", "GetName", "tests/fixtures/csharp"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].scope, "MyApp.Models.User.GetName");
}

// === Comprehensive fixture tests ===

#[test]
fn peek_for_csharp_comprehensive_file_scoped_class() {
    let output = peek(&["-k", "class", "EmailService", "tests/fixtures/csharp"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    // EmailService exists in comprehensive.cs (file-scoped namespace Comprehensive.Services)
    // and no other EmailService in sample.cs
    let results = parse_defs(&stdout);
    assert!(
        results
            .iter()
            .any(|r| r.scope == "Comprehensive.Services.EmailService" && r.kind == "class")
    );
}

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
    // "Create" verifies constructor-like scope behavior.
    let output = peek(&["-k", "function", "Create", "tests/fixtures/csharp"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(
        results
            .iter()
            .any(|r| r.scope == "MyApp.Models.User.Create" && r.kind == "function")
    );
}
