// Comprehensive C fixture -- covers all 5 DefKind: Function, Struct, Enum, Type, Const

// === Function ===

int process(int x) {
    return x * 2;
}

static void helper(void) {
    // static function
}

char *dup_str(const char *s) {
    return ((void*)0);
}

// === Struct ===

struct Point {
    double x;
    double y;
};

struct Config {
    int timeout;
    int retries;
};

// === Enum ===

enum Color {
    RED,
    GREEN,
    BLUE
};

enum Status {
    OK = 0,
    ERROR = 1
};

// === Type (typedef) ===

typedef int StatusCode;

typedef char *StringPtr;

typedef struct Node Node;

// === Const ===

const int MAX_SIZE = 1024;

static const int VERSION = 2;

const char *MSG = "hello";

// === Definitions inside function body (should NOT be extracted) ===

void withLocalDefs(void) {
    const int LOCAL_CONST = 42;
}
