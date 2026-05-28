// core.java — all kind classifications, scope paths, signature formats
// Each construct appears once; no duplicate coverage across core and edge.

package com.example;

// ── Class ──
public class MyClass {
    // ── Field ──
    private int count;
    private int field;

    // ── Const (static final) ──
    public static final int MAX_SIZE = 100;

    // ── Constructor ──
    public MyClass(int count) {
        this.count = count;
    }

    // ── Method ──
    public int getCount() {
        return count;
    }

    // ── Nested class (scope pattern) ──
    static class Builder {
        private String name;
    }
}

// ── Interface ──
interface Drawable {
    // ── MethodDeclaration (abstract method, no body) ──
    void draw();
}

// ── Enum ──
enum Status {
    // ── Variant ──
    ACTIVE, INACTIVE
}

// ── Record ──
record Point(int x, int y) {}

// ── Annotation ──
@interface CacheConfig {
    // ── MethodDeclaration (annotation element) ──
    String value();
}