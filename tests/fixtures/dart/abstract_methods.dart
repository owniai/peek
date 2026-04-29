// Dart test fixture for abstract methods (methods without body)
// These are declared inside abstract classes and end with `;` instead of `{}`

abstract class Shape {
  // Abstract method - no body, ends with semicolon
  double area();
  double perimeter();

  // Concrete method in abstract class - has body
  String describe() => "Shape with area=\${area()}";
}

abstract class Repository<T> {
  // Abstract method with generic type
  Future<T> findById(int id);
  Future<List<T>> findAll();

  // Concrete method
  void logAccess(int id) {
    print("Accessed id: \$id");
  }
}

// Interface-style abstract class (Dart 3 `interface class` modifier)
abstract interface class Comparable<T> {
  int compareTo(T other);
}
