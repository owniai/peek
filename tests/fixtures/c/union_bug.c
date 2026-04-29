// Bug fixture: union definitions are not recognized by the C parser.
// The C parser's collect_definitions() only handles "struct_specifier" but not
// "union_specifier", even though tree-sitter-c produces a union_specifier node
// with the same structure (name, body fields) as struct_specifier.

// This union should be discoverable as a Struct definition.
union Value {
    int i;
    float f;
    double d;
};

// Nested union inside a struct -- the struct is found but union is not.
struct Container {
    int type;
    union Data {
        int int_val;
        float float_val;
    } data;
};

// Anonymous union typedef -- the typedef name is found as Type,
// but the union itself is not discovered.
typedef union {
    void *ptr;
    int handle;
} Handle;
