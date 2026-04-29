// tests/fixtures/java/Comprehensive.java
package com.example;

// Class with static final constants and methods
public class Constants {
    public static final int MAX_SIZE = 100;
    public static final String VERSION = "1.0";
    private static final double PI = 3.14159;

    public int getMaxSize() {
        return MAX_SIZE;
    }
}

// Class with method overloading
class Calculator {
    int add(int a, int b) {
        return a + b;
    }

    double add(double a, double b) {
        return a + b;
    }

    static int multiply(int a, int b) {
        return a * b;
    }
}

// Enum with constructor and method
public enum Color {
    RED("#FF0000"),
    GREEN("#00FF00"),
    BLUE("#0000FF");

    private final String hex;

    Color(String hex) {
        this.hex = hex;
    }

    public String getHex() {
        return hex;
    }
}

// Interface with abstract, default, and static methods
// Named Renderable (not Drawable) to avoid collision with Sample.java
public interface Renderable {
    void render();

    default void resize() {
        System.out.println("resizing");
    }

    static void factory() {}
}

// Interface with constants
interface Config {
    String NAME = "default";
    int MAX_RETRIES = 3;

    void configure();
}
