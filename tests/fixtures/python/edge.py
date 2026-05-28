# edge.py — boundary behaviors: function-body exclusion, dunder at module level,
# Enum/Protocol classification edge cases, TypeAlias annotated form, false-positive resilience,
# Field vs Var vs Alias distinction, same-name disambiguation, multiline signature with comments

# ── Dunder at module level stays Function (not Constructor/Operator) ──
def __init__():
    pass


def __add__(x, y):
    pass


# ── Function-body definitions NOT extracted ──
def factory():
    local_var = 1
    count: int = 0
    def inner():
        pass


# ── TypeAlias annotated form ──
HeaderValue: TypeAlias = str | list[str]


# ── TypeAlias in class NOT Field ──
class AppConfig:
    Value: TypeAlias = dict[str, object]


# ── Enum classification edge cases ──
class Status(SomeMixin, IntEnum):
    OK = 0


class Perm(Flag):
    R = 1


# ── Protocol classification edge cases ──
class MyProto(SomeBase, Protocol):
    def method(self): ...


# ── Enum priority over Protocol ──
class Weird(Enum, Protocol):
    A = 1


# ── Non-Enum/Protocol base → stays Class ──
class ChildClass(Parent):
    def method(self):
        pass


# ── Plain annotation NOT TypeAlias → Var ──
name: str = "hello"


# ── Plain assignment NOT TypeAlias → Var ──
MAX_SIZE = 100


# ── False-positive resilience ──
# tree-sitter AST handles: string/comment/triple-quote never parsed as def/class
def real_func():
    pass


code_string = "def fake_in_string():\n    pass"
# class FakeInComment: pass
doc = '''
def fake_in_triple_quotes():
    pass
class AlsoFake:
    pass
'''


# ── Minimal one-line class ──
class MinimalClass: pass


# ── Multiline signature with inline comments ──
def process_data(
    items: list,  # input items
    verbose: bool,  # verbose mode
) -> None:
    pass


# ── Decorated class with comment ──
@dataclass  # auto-generate __init__
class UserRecord:
    name: str
    age: int


# ── Decorated function with comment on decorator line ──
@retry(max_attempts=3)  # retry up to 3 times
def fetch_url(url: str) -> bytes:
    pass


# ── Stacked decorators with comments ──
@cache  # cache the result
@timeout(30)  # 30 second timeout
def compute(x: int) -> int:
    return x


# ── Multiline type alias ──
type Matrix = list[
    list[float],
]