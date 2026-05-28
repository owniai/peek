# core.py — all kind classifications, scope paths, signature formats, nested scope
# Each construct appears once; no duplicate coverage across core and edge.

# ── Function ──
def top_level_func(x: int) -> str:
    pass


def _private():
    pass


async def async_fetch(url: str) -> None:
    pass


# ── Var (module-level) ──
config_path = '/etc/app'

max_retries: int = 3

debug_mode: bool


# ── Class ──
class MyClass(Parent):
    # ── Field ──
    name = 'default'
    timeout: int = 30

    # ── Method ──
    def regular_method(self):
        pass

    # ── Constructor ──
    def __init__(self, name):
        pass

    # ── Destructor ──
    def __del__(self):
        pass

    # ── Operator ──
    def __add__(self, other):
        pass

    # ── Subscript ──
    def __getitem__(self, key):
        pass

    # ── Getter ──
    @property
    def count(self):
        pass

    # ── Setter ──
    @count.setter
    def count(self, value):
        pass

    # ── Nested class ──
    class Inner:
        def inner_method(self):
            pass


class StaticHolder:
    @staticmethod
    def static_helper():
        pass


class ClassMeta:
    @classmethod
    def factory_method(cls):
        pass


class AsyncService:
    async def async_handle(self):
        pass


# ── Enum ──
class Color(Enum):
    RED = 1


# ── Protocol ──
class Serializable(Protocol):
    def serialize(self) -> bytes: ...


# ── Alias (PEP 695 type statement) ──
type Point = tuple[float, float]

type Result[T] = T | None


# ── Alias in class ──
class Settings:
    type Config = dict[str, object]


# ── Deep nested scope ──
class L1:
    class L2:
        class L3:
            class L4:
                class L5:
                    def deep_method(self):
                        pass


# ── Same-name in different scopes ──
def process():
    pass


class Alpha:
    def process(self):
        pass


class Beta:
    def process(self):
        pass


class First:
    class Item:
        pass


class Second:
    class Item:
        pass