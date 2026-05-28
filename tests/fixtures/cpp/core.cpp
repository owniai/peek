// core.cpp — all kind classifications, scope paths, signature formats, nested scope
// Each construct appears once; no duplicate coverage across core and edge.

// ── Function ──
int process(int x) {
    return x * 2;
}

// ── Var ──
int global_count = 0;

// ── Static Var ──
static int file_count = 0;

static char *file_name;

// ── Const ──
const int MAX_SIZE = 1024;

constexpr int MAX_THREADS = 16;

// ── Macro ──
#define DEBUG_FLAG 1

// ── Alias (typedef) ──
typedef int StatusCode;

typedef int Handle;

// ── Alias (using) ──
using Callback = void(*)(int);

using Processor = void(*)(int);

using std::vector;

// ── Namespace ──
namespace Core {

// ── Alias (namespace alias) ──
namespace io = Core;

// ── Function in namespace ──
void run() {}

// ── Class ──
class Service {
public:
    // ── Constructor ──
    Service(int port) {}

    // ── Destructor ──
    ~Service() {}

    // ── Method ──
    void execute() {}

    // ── Operator ──
    bool operator==(const Service& o) const { return true; }

    // ── Field ──
    int port;
};

// ── Struct in namespace ──
struct Config {
    int timeout;
};

// ── Enum in namespace ──
enum Status {
    OK,
    ERROR
};

// ── Const in namespace ──
const int TIMEOUT = 30;

// ── Static Var in namespace ──
static int counter = 0;

} // namespace Core

// ── Struct ──
struct Point {
    double x;
    double y;
};

// ── Union ──
union Data {
    int i;
    float f;
};

// ── Enum ──
enum Color {
    RED,
    GREEN,
    BLUE
};

// ── Enum Class ──
enum class Direction : int {
    UP,
    DOWN
};

// ── Typedef Enum ──
typedef enum {
    TOGGLE_ON,
    TOGGLE_OFF
} Toggle;

// ── Variant ── (enum values are Variant kind, covered above via RED/GREEN/BLUE etc.)

// ── Concept ──
template<typename T>
concept Printable = requires(T t) { t.print(); };

// ── Deep nested scope ──
namespace App {
namespace Detail {

void compute() {}

const int BUFFER_SIZE = 4096;

class Container {
public:
    class Item {
    public:
        void validate() {}
    };
};

} // namespace Detail
} // namespace App