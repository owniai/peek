// edge.swift — boundary behaviors: operator declarations, precedencegroup,
// protocol method declaration, macro, subscript declaration, actor init,
// init not matched by function filter, function-body NOT extracted,
// async/throws/static in signatures

// ── OperatorDeclaration (infix/prefix/postfix) ──
infix operator +++: AdditionPrecedence

prefix operator !!!

postfix operator ~~~

// ── Operator (precedencegroup) ──
precedencegroup MultiplicativePrecedence {
    associativity: left
    higherThan: AdditivePrecedence
}

// ── Protocol operator requirements ──
protocol EquatableByValue {
    static func ==(lhs: Self, rhs: Self) -> Bool
    static func !=(lhs: Self, rhs: Self) -> Bool
    func normalize() -> Self
}

// ── SubscriptDeclaration (protocol requirement, no body) ──
protocol Table {
    subscript(row: Int) -> String { get }
}

// ── ConstructorDeclaration (protocol init requirement, no body) ──
protocol Factory {
    init(name: String)
}

// ── Macro ──
macro stringify<T>(_ value: T) -> String

// ── Actor init (init not matched by function filter) ──
actor TaskRunner {
    init() {
        setup()
    }
}

// ── Function-body definitions NOT extracted ──
func factory() {
    let localConst = 1
    var localVar = 0
    func inner() {}
}

// ── Async/throws/static in signatures ──
class AsyncService {
    static func helper() -> Void {}
    func process() async throws -> Data {}
}

// ── Same-name in different scopes ──
func process() {}

class Alpha {
    func process() {}
}

class Beta {
    func process() {}
}