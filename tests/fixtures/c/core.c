// core.c — all kind classifications, scope patterns
// Each construct appears once; no duplicate coverage across core and edge.

// ── Function ──
int compute(int n) {
    return n * 2;
}

static void helper(void) {
    // static function — tested for static signature
}

char *dup_str(const char *s) {
    return ((void*)0);
}

int process(int x) {
    return x * 2;
}

// ── Struct ──
struct Vec2 {
    double dx;
    double dy;
};

struct Point {
    double x;
    double y;
};

struct Config {
    int timeout;
    int retries;
};

struct Outer {
    struct Inner { int a; int b; } data;
    struct { int x; int y; } anon;
    union { int i; float f; } tagged;
};

// ── Union ──
union TagVal {
    int num;
    float val;
};

union Value {
    int i;
    float f;
    double d;
};

struct Container {
    int type;
    union Data {
        int int_val;
        float float_val;
    } data;
};

// ── Enum ──
enum Dir {
    NORTH,
    SOUTH,
    EAST,
    WEST
};

enum Color {
    RED,
    GREEN,
    BLUE
};

enum Status {
    OK = 0,
    ERROR = 1
};

// ── Alias (typedef) ──
typedef int Handle;

typedef int StatusCode;

typedef char *StringPtr;

typedef struct Node Node;

// ── Typedef Enum ──
typedef enum {
    SWITCH_ON,
    SWITCH_OFF
} Switch;

// ── Const ──
const int LIMIT = 100;

const int MAX_SIZE = 1024;

const char *MSG = "hello";

// ── Macro ──
#define DEBUG 1

// ── Var ──
static int refs = 0;

static int file_count = 0;

static char *file_name;

// ── Function-body definitions (should NOT be extracted) ──
void withLocalDefs(void) {
    const int LOCAL_CONST = 42;
}