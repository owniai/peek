// Comprehensive C++ fixture -- covers all 6 DefKind: Function, Class, Struct, Enum, Type, Const
// Plus namespace/class scope nesting with `::` separator

// === Top-level Function ===

int process(int x) {
    return x * 2;
}

// === Top-level Class ===

class Engine {
public:
    void start() {}
    void stop() {}
};

// === Top-level Struct ===

struct Point {
    double x;
    double y;
};

// === Top-level Enum ===

enum Color {
    RED,
    GREEN,
    BLUE
};

enum class Direction : int {
    UP,
    DOWN
};

// === Top-level Type ===

typedef int StatusCode;

using Callback = void(*)(int);

// === Top-level Const ===

const int MAX_SIZE = 1024;

constexpr int MAX_THREADS = 16;

// === Namespace with nested definitions ===

namespace Core {

// Function in namespace
void run() {}

// Class in namespace
class Service {
public:
    void execute() {}
};

// Struct in namespace
struct Config {
    int timeout;
};

// Enum in namespace
enum Status {
    OK,
    ERROR
};

// Type in namespace
typedef int Handle;

using Processor = void(*)(int);

// Const in namespace
const int TIMEOUT = 30;

// Static variable in namespace
static int counter = 0;

// Nested class inside class
class Container {
public:
    class Item {
    public:
        void validate() {}
    };
};

} // namespace Core

// === Nested namespace ===

namespace App {
namespace Detail {

void compute() {}

const int BUFFER_SIZE = 4096;

} // namespace Detail
} // namespace App

// === Static variables at file scope ===

static int file_count = 0;

static char *file_name;
