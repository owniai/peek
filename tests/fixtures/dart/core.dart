// core.dart — all kind classifications, scope paths, signature formats, nested scope
// Each construct appears once; no duplicate coverage across core and edge.

library my_lib;

// ── Function ──
void topFunc(String msg) {}

// ── Var ──
int counter = 0;

// ── Const ──
const MAX_LIMIT = 100;

// ── Alias ──
typedef Callback = void Function(String);

// ── Class ──
@deprecated
class UserService {
  String role;
  UserService(this.role);
  UserService.admin() : role = 'admin';
  factory UserService.guest() => UserService('guest');

  String greet(String greeting) {
    return '$greeting, $role!';
  }

  String get displayName => role.toUpperCase();

  set displayName(String value) {
    role = value;
  }

  bool operator ==(Object other) => other is UserService && role == other.role;
}

// ── Enum ──
enum Status { active }

// ── Mixin ──
mixin Loggable {
  void log(String msg) => print(msg);
}

// ── Interface ──
interface class Serializable {}

// ── Extension ──
extension StringExt on String {
  String repeated(int n) => this * n;
}

// ── ExtensionType ──
extension type Email(String value) {
  String get domain => value.split('@').last;
}

// ── Abstract class ──
abstract class BaseProcessor {
  void process();

  String describe() => "Processor";
}

// ── Same-name in different scopes ──
void process() {}

class Alpha {
  void process() {}
}

class Beta {
  void process() {}
}