// core.hpp — C++ header fixture (parsed by cplusplus parser)
// Core kind classifications in header context: function prototypes, class/struct,
// namespace, typedef, using, constexpr, enum class, concept, extern

#ifndef CORE_HPP_
#define CORE_HPP_

// ── FunctionDeclaration (prototype) ──
int core_process(int x);
void core_run(void);

// ── Const ──
constexpr int CORE_MAX_THREADS = 16;

// ── Macro ──
#define CORE_DEBUG_FLAG 1

// ── Alias (typedef) ──
typedef int CoreStatusCode;
typedef int CoreHandle;

// ── Alias (using) ──
using CoreCallback = void(*)(int);

// ── Namespace ──
namespace CoreNs {

// ── Namespace alias ──
namespace io = CoreNs;

// ── FunctionDeclaration in namespace ──
void dispatch(int signal);

// ── Class ──
class CoreService {
public:
    CoreService(int port);
    ~CoreService();

    void execute();
    bool operator==(const CoreService& o) const;

    int port;
};

// ── Struct in namespace ──
struct CoreConfig {
    int timeout;
};

// ── Enum in namespace ──
enum CoreStatus {
    CORE_OK,
    CORE_ERROR
};

// ── Const in namespace ──
const int CORE_TIMEOUT = 30;

} // namespace CoreNs

// ── Struct ──
struct CorePoint {
    double x;
    double y;
};

// ── Union ──
union CoreData {
    int i;
    float f;
};

// ── Enum ──
enum CoreColor {
    CORE_RED,
    CORE_GREEN,
    CORE_BLUE
};

// ── Enum Class ──
enum class CoreDirection : int {
    UP,
    DOWN
};

// ── Concept ──
template<typename T>
concept CorePrintable = requires(T t) { t.print(); };

// ── Extern ──
extern int core_global_ref;

#endif // CORE_HPP_