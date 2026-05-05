// Dart comprehensive test fixture for peek
// Covers: class, abstract class, enum, mixin, extension, typedef, const, function,
//         getter/setter, constructor, factory constructor, nested scope, annotations

// Top-level function
void globalHelper(String message) {
  print(message);
}

// Top-level constants
const APP_VERSION = "1.0.0";
const int MAX_RETRIES = 3;

// Type alias
typedef Callback = void Function(String);

// Simple class with annotation
@deprecated
class UserService {
  final String name;

  // Constructor
  UserService(this.name);

  // Factory constructor
  factory UserService.admin() => UserService("admin");

  // Method
  String greet(String greeting) {
    return "$greeting, $name!";
  }

  // Getter
  String get displayName => name.toUpperCase();

  // Setter
  set displayName(String value) {
    // setter logic
  }

  // Static const
  static const int DEFAULT_TIMEOUT = 30;
}

// Abstract class
abstract class BaseProcessor {
  void process();

  String describe() => "Processor";
}

// Enum
enum Status { active, inactive, pending }

// Mixin
mixin Loggable {
  void log(String message) {
    print(message);
  }
}

mixin Validatable {
  bool validate() => true;
}

// Extension
extension StringExt on String {
  String repeated(int times) => this * times;

  String get capitalized =>
      isEmpty ? '' : '${this[0].toUpperCase()}${substring(1)}';
}

// Extension type
extension type Point(double x, double y) {
  Point.origin() : this(0.0, 0.0);

  double distanceTo(Point other) {
    return (x - other.x).abs() + (y - other.y).abs();
  }
}

// Class with mixin
class ApiClient extends BaseProcessor with Loggable {
  @override
  void process() {
    log("Processing...");
  }
}

// Multi const declaration
const CACHE_TTL = 300, MAX_ITEMS = 1000;

// Definitions inside function body (should NOT be extracted)
void withLocalDefs() {
  const localConst = 42;
  void localHelper() {
    print("local");
  }
}

// Class with various fields
class Product {
  int id;
  String label;
  static int instanceCount = 0;
  final double price;

  Product(this.id, this.label, this.price);
}
