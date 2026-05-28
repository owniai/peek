// edge.java — boundary behaviors: throws clause in signature, annotation preservation,
// enum constructor not Class, interface method as MethodDeclaration, annotation element as MethodDeclaration,
// overloaded methods, deeply nested scope, function-body NOT extracted

// ── throws clause in abstract method signature ──
abstract class ThrowsAbstract {
    abstract void readData() throws IOException, SQLException;
}

// ── throws clause in interface method signature ──
interface Processor {
    void process(String input) throws IllegalArgumentException;
}

// ── Annotation preservation in signature ──
@Deprecated
class AnnotatedClass {}

@FunctionalInterface
interface AnnotatedInterface {
    void compute();
}

// ── Enum constructor is Constructor, not Class ──
enum Color {
    RED, GREEN, BLUE;

    private Color() {}
}

// ── Interface default method is Method (has body) ──
interface Renderable {
    void render();

    default void resize() {
        System.out.println("resizing");
    }

    static void factory() {}
}

// ── Annotation element with default value ──
@interface Config {
    int ttl() default 3600;
}

// ── Overloaded methods ──
class Calculator {
    int add(int a, int b) {
        return a + b;
    }

    double add(double a, double b) {
        return a + b;
    }
}

// ── Deeply nested scope ──
class L1 {
    class L2 {
        class L3 {
            void deepMethod() {}
        }
    }
}

// ── Function-body NOT extracted (inner class in method body) ──
class BodyExclusion {
    void methodWithLocal() {
        // class InnerClass {} ← should NOT be extracted
    }
}

// ── Same-name in different scopes ──
class Alpha {
    void process() {}
}

class Beta {
    void process() {}
}

// ── Nested interface inside class ──
class Container {
    interface Serializable {
        byte[] serialize();
    }
}

// ── Nested enum inside class ──
class Holder {
    enum Priority {
        LOW
    }
}

// ── Nested record inside class ──
class Box {
    record Pair(String first, String second) {}
}

// ── Multiple fields in one declaration ──
class MultiField {
    int x, y;
}

// ── Record with compact constructor and body method ──
public record Range(int min, int max) {
    public Range {
        if (min > max) throw new IllegalArgumentException();
    }

    public int span() {
        return max - min;
    }
}

// ── Interface constant (constant_declaration, implicitly public static final) ──
interface Settings {
    String NAME = "default";
}