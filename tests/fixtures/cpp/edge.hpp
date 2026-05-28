// edge.hpp — C++ header fixture (parsed by cplusplus parser)
// Tests: header-specific constructs — include guards, function prototypes,
// class forward declarations, template declarations, constexpr, inline,
// namespace-scoped declarations, static_assert, concept declarations

#ifndef EDGE_HPP_
#define EDGE_HPP_

// ── FunctionDeclaration (prototype, no body) ──
int hpp_init(int mode);
void hpp_cleanup(void);
inline int hpp_fast_path(int x);

// ── ClassDeclaration (forward declaration) ──
class Handle;
class Allocator;

// ── Template function declaration ──
template<typename T>
T hpp_convert(T input);

// ── Template class declaration ──
template<typename T, int N>
class HppArray {
public:
    T data[N];
    int size() const;
};

// ── Constexpr function declaration ──
constexpr int hpp_limit();

// ── Namespace with declarations ──
namespace HppNs {
    void dispatch(int signal);
    const int HPP_VERSION = 2;
    class Dispatcher;
}

// ── Concept (C++20) ──
template<typename T>
concept HppComparable = requires(T a, T b) { a < b; };

// ── static_assert ──
static_assert(sizeof(int) == 4, "int must be 4 bytes");

// ── Extern declarations ──
extern int hpp_global_ref;
extern const char *hpp_lib_name;

// ── Using alias ──
using HppResult = int;

// ── Macro (define) ──
#define HPP_MAX_ENTRIES 256

#endif // EDGE_HPP_