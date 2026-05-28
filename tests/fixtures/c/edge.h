// edge.h — C header fixture (parsed by C++ tree-sitter, language = cplusplus)
// Tests: header-specific constructs — include guards, function prototypes,
// struct declarations, typedef, extern declarations, macro definitions

#ifndef EDGE_H_
#define EDGE_H_

// ── FunctionDeclaration (prototype, no body) ──
int header_init(void);
void header_cleanup(int code);
const char *get_version(void);

// ── StructDeclaration (forward declaration) ──
struct ListNode;
struct FileHandle;

// ── Struct with fields ──
struct HeaderConfig {
    int max_retries;
    int timeout_ms;
};

// ── Typedef ──
typedef int StatusCode;
typedef struct ListNode *ListIter;

// ── Macro (define) ──
#define HEADER_VERSION "1.0"
#define MAX_HEADER_SIZE 4096

// ── Extern declarations ──
extern int global_count;
extern const char *app_name;

#endif // EDGE_H_