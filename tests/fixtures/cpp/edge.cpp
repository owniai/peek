// edge.cpp — boundary behaviors: forward declarations, extern declarations,
// out-of-class definitions, =default/=delete, anonymous struct/union,
// function-body NOT extracted, operator overloading, multi-line signature

// ── Forward declarations (ClassDeclaration, StructDeclaration, EnumDeclaration) ──
class Node;

struct Data;

enum Color;

// ── Forward declaration with enum class ──
enum class Status : int;

// ── Union forward declaration (UnionDeclaration) ──
union Packet;

// ── Function declaration (FunctionDeclaration) ──
int compute(int x);

// ── Extern VarDeclaration ──
extern int shared_counter;

// ── Extern ConstDeclaration ──
extern const int EXTERN_CONST;

// ── Extern var with initializer is still Var ──
extern int initialized_var = 5;

// ── Out-of-class method definition ──
class Engine {
public:
    void start();
    void stop();
};

void Engine::start() {
    // implementation
}

void Engine::stop() {
    // implementation
}

// ── Function returning const type (const function prototype) ──
const int compute();

// ── Out-of-class destructor definition ──
class Foo {
public:
    ~Foo();
};

Foo::~Foo() {}

// ── Out-of-class operator definition ──
class Vec2 {
public:
    Vec2 operator+(const Vec2& o) const;
};

Vec2 Vec2::operator+(const Vec2& o) const { return Vec2(); }

// ── =default constructor ──
class Defaulted {
public:
    Defaulted() = default;
};

// ── =delete method ──
class Deleted {
public:
    void bar() = delete;
};

// ── Anonymous struct/union field ──
struct Outer {
    struct { int x; } anon;
    union { int i; float f; } data;
};

// ── Function-body definitions NOT extracted ──
void factory() {
    int local_var = 1;
    // inner definitions inside function body should not be extracted
}

// ── Multi-line signature ──
void process_data(
    int items,
    bool verbose
) {
    // implementation
}

// ── Linkage specification ──
extern "C" {
    void c_func(int x);
}

// ── Explicit constructor declaration (ConstructorDeclaration) ──
class Config {
public:
    explicit Config(const char* path);
};

// ── Pure virtual method declaration (MethodDeclaration) ──
class Abstract {
public:
    virtual void do_work() = 0;
};

// ── Static method declaration ──
class StaticHolder {
public:
    static void helper();
};