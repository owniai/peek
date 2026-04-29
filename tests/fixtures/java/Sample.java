// tests/fixtures/java/Sample.java
package com.example;

import java.util.List;

// Top-level class with various nested types
public class MyClass {
    private int field;

    // Static inner class
    public static class Builder {
        private String name;

        // 3-level nesting for scope test
        public class Config {
            private boolean debug;
        }
    }

    // Non-static inner class
    class InnerHelper {
        void helperMethod() {}
    }

    // Inner interface
    interface Serializable {
        byte[] serialize();
    }

    // Inner enum
    public enum Priority {
        LOW, MEDIUM, HIGH
    }
}

// Top-level interface
interface Drawable {
    void draw();
}

// Top-level enum
enum Status {
    ACTIVE, INACTIVE
}

// Abstract class
abstract class Shape {
    abstract double area();
}
