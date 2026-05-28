// edge.c — boundary behaviors: declaration vs definition, nested types,
// typedef struct no double extraction, macro inside struct/union,
// function-body definitions NOT extracted, multi-line flattening,
// forward declarations, extern declarations, static const vs var,
// typedef union

// ── FunctionDeclaration (prototype) vs Function (definition) ──
int init(void);

int init(void) {
    return 0;
}

// foo: prototype + definition — tests declaration/definition distinction
int foo(void);

int foo(void) {
    return 42;
}

// add: definition only — should NOT match -k function_declaration
int add(int a, int b) {
    return a + b;
}

static void helper(int x) {
    (void)x;
}

// ── static+const as Const (not Var) ──
static const int CONF_VERSION = 2;
static const int VERSION = 2;

// ── StructDeclaration (forward declaration) ──
struct Link;

// ── UnionDeclaration ──
union Payload;

// ── EnumDeclaration ──
enum Flag;

// ── ConstDeclaration (extern const without initializer) ──
extern const int MAX_LIMIT;

// ── VarDeclaration (extern without initializer) ──
extern int total;

// ── Nested struct/union inside struct ──
struct Mixed {
    int kind;
    union InnerData {
        int int_val;
        float float_val;
    } data;
};

// ── typedef struct no double extraction ──
typedef struct Pair { int first; int second; } PairT;

// ── typedef union (anonymous union typedef) ──
typedef union {
    void *ptr;
    int handle;
} AnonHandle;

// ── macro inside struct ──
struct WithMacro {
#define INNER_MACRO 42
    int x;
};

// ── macro inside union ──
union UnionMacro {
#define UNION_MACRO 1
    int i;
    float f;
};

// ── function-body definitions NOT extracted ──
void outer_func(void) {
    const int LOCAL_CONST_EDGE = 42;
}

// ── Multi-line signature flattening ──
int multi_line_func(int a,
                    int b,
                    int c) {
    return a + b + c;
}