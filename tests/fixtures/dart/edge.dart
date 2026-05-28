// edge.dart — boundary behaviors: dual-kind classification, operator overloading variants,
// abstract method as MethodDeclaration, external declaration signature prefix,
// extension type, late final as Const, field vs var, enum constant as Variant,
// function-body NOT extracted

// ── mixin class as dual-kind (Class+Mixin) ──
mixin class Draggable {
  void drag() {}
}

// ── abstract interface class as dual-kind (Class+Interface) ──
abstract interface class Comparable<T> {
  int compareTo(T other);
}

// ── Operator: subscript ──
class Matrix {
  int operator [](int i) => 0;
  void operator []=(int i, int v) {}
}

// ── Operator: unary ──
class Flags {
  int operator ~() => 0;
}

// ── Operator: binary ──
class Vector {
  Vector operator +(Vector other) => this;
}

// ── Operator: comparison ──
class Money {
  bool operator ==(Object other) => true;
}

// ── Abstract method as MethodDeclaration ──
abstract class Shape {
  double area();
}

// ── External declaration signature prefix ──
external void externalFunc(int x);
external int get externalGetter;
external void set externalSetter(int v);

class NativeLib {
  external void extMethod(int code);
  external int get extGetter;
  external factory NativeLib._internal();
}

// ── ExtensionType with constructor and method ──
extension type Point(double x, double y) {
  Point.origin() : this(0.0, 0.0);
  double distanceTo(Point other) => 0;
}

// ── late final as Const ──
late final String configName;

// ── class field vs top-level var ──
class Product {
  int id;
  String label = 'item';
}

int count = 0;

// ── enum constant as Variant ──
enum Color { red }

// ── function-body NOT extracted ──
void withLocalDefs() {
  const localConst = 42;
  void localHelper() {
    print("local");
  }
}