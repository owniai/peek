// core.h — C header fixture (parsed by cplusplus parser)
// Core kind classifications in header context: function prototypes, struct definitions,
// typedef, enum, const, macro, extern declarations

#ifndef CORE_H_
#define CORE_H_

// ── FunctionDeclaration (prototype) ──
int core_init(int mode);
static void core_helper(int x);
char *core_dup_str(const char *s);

// ── Struct ──
struct CoreVec2 {
    double dx;
    double dy;
};

struct CorePoint {
    double x;
    double y;
};

struct CoreConfig {
    int timeout;
    int retries;
};

// ── Union ──
union CoreTagVal {
    int num;
    float val;
};

// ── Enum ──
enum CoreDir {
    CORE_NORTH,
    CORE_SOUTH,
    CORE_EAST,
    CORE_WEST
};

enum CoreColor {
    CORE_RED,
    CORE_GREEN,
    CORE_BLUE
};

// ── Alias (typedef) ──
typedef int CoreHandle;
typedef int CoreStatusCode;
typedef char *CoreStringPtr;

typedef struct CoreNode CoreNode;

// ── Typedef Enum ──
typedef enum {
    CORE_SWITCH_ON,
    CORE_SWITCH_OFF
} CoreSwitch;

// ── Const ──
const int CORE_LIMIT = 100;
const int CORE_MAX_SIZE = 1024;

// ── Macro ──
#define CORE_DEBUG 1

// ── Extern VarDeclaration ──
extern int core_global_count;
extern const char *core_app_name;

#endif // CORE_H_