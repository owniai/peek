# Large-scale stress test fixture for tree-sitter-python parsing.
#
# Total expected definitions:
#   Functions (def / async def): 51
#     - Section 1 (module-level): 13
#     - Section 2 (simple class methods): 13
#     - Section 3 (inheritance methods): 9
#     - Section 4 (nested class methods): 6
#     - Section 5 (decorated class methods): 4
#     - Section 6 (scope_resolution): 6
#   Classes: 30
#     - Section 2 (simple): 5
#     - Section 3 (inheritance): 6
#     - Section 4 (nested): 9
#     - Section 5 (decorated): 3
#     - Section 6 (scope_resolution): 5
#     - Outer module-level classes: 2 (OuterConfig, ModuleRegistry counted in nested)
#
# Note: Section 6 contains definitions with the same name in different scopes.
# Those are expected to return multiple results when queried by name.

from dataclasses import dataclass
from typing import List, Optional, Dict


# =============================================================================
# Section 1: Module-level functions (14 functions)
# =============================================================================

def utility_plain():
    pass


def compute(a, b, c):
    return a + b + c


def typed_transform(value: int, factor: float) -> float:
    return value * factor


def full_signature(name: str, age: int = 0, tags: Optional[List[str]] = None) -> Dict[str, object]:
    return {"name": name}


async def async_load(url: str) -> bytes:
    pass


async def async_process(items: List[Dict]) -> List[Dict]:
    pass


async def async_cleanup() -> None:
    pass


@staticmethod
def standalone_static():
    pass


def _internal_helper(x: int) -> int:
    return x * 2


def __dunder_module_init__():
    pass


@retry(max_attempts=3)
def retried_operation(data: bytes):
    pass


@timeout(60)
@log_calls
def stacked_decorated_task(query: str):
    pass


@deprecated("use typed_transform instead")
def legacy_transform(value):
    return value


# =============================================================================
# Section 2: Simple classes with methods (5 classes, 12 methods)
# =============================================================================

class Calculator:
    def add(self, a: int, b: int) -> int:
        return a + b

    def multiply(self, a: float, b: float) -> float:
        return a * b

    @staticmethod
    def identity(x):
        return x


class Repository:
    def find_by_id(self, entity_id: int):
        pass

    @classmethod
    def from_config(cls, config: Dict):
        pass


class Formatter:
    def format(self, value: object) -> str:
        return str(value)

    def parse(self, text: str) -> object:
        pass


class CacheManager:
    _store: Dict[str, object] = {}

    def get(self, key: str):
        pass

    def invalidate(self) -> None:
        pass

    @classmethod
    def create_default(cls):
        pass


class Validator:
    def check(self, value: object) -> bool:
        pass

    @staticmethod
    def is_valid_email(email: str) -> bool:
        pass


# =============================================================================
# Section 3: Inheritance classes (4 classes, 8 methods)
# =============================================================================

class BaseHandler:
    def handle(self, request):
        pass

    def on_error(self, error):
        pass


class ApiHandler(BaseHandler):
    def handle(self, request):
        pass

    def format_response(self, data):
        pass


class ReadMixin:
    def read(self, path: str) -> bytes:
        pass


class WriteMixin:
    def write(self, path: str, data: bytes) -> None:
        pass


class FileHandler(ReadMixin, WriteMixin):
    def read(self, path: str) -> bytes:
        pass

    def copy(self, src: str, dst: str) -> None:
        pass


class StreamingHandler(FileHandler, BaseHandler):
    def stream(self, source):
        pass


# =============================================================================
# Section 4: Nested classes (3 groups, 9 classes, 5 methods)
# =============================================================================

# --- 2-layer nested ---

class OuterConfig:
    class Defaults:
        TIMEOUT = 30

    class Overrides:
        def apply(self, config: Dict):
            pass


# --- 3-layer nested ---

class ModuleRegistry:
    class Core:
        class Engine:
            def start(self):
                pass

            def stop(self):
                pass

    class Plugins:
        class Loader:
            def load(self, name: str):
                pass


# --- 2-layer nested with siblings ---

class ServiceContainer:
    class ServiceA:
        def initialize(self):
            pass

    class ServiceB:
        def initialize(self):
            pass


# =============================================================================
# Section 5: Decorated classes (3 classes, 4 methods)
# =============================================================================

@dataclass
class DataPoint:
    x: float
    y: float
    label: str = ""

    def distance_to(self, other: "DataPoint") -> float:
        pass


@frozen
class ImmutableConfig:
    host: str
    port: int

    def to_url(self) -> str:
        pass


@registered("observer")
class EventObserver:
    def on_event(self, event):
        pass

    @classmethod
    def listen_to(cls, *event_types):
        pass


# =============================================================================
# Section 6: Scope resolution - same name in different scopes
# (3 groups: 3 name collisions for functions, 2 for classes)
# =============================================================================

# "initialize" appears here, in ServiceContainer.ServiceA, and ServiceContainer.ServiceB
# Expected: 3 results when querying "initialize" -> scope=initialize, ServiceContainer.ServiceA.initialize, ServiceContainer.ServiceB.initialize
def initialize():
    pass


# "Config" appears here and as ImmutableConfig alias above (not a collision).
# "transform" appears here and inside Transformer below.
# Expected: 2 results when querying "transform" -> scope=transform, Transformer.transform
def transform(data):
    pass


class Transformer:
    def transform(self, input_data):
        pass

    def validate(self):
        pass


# "validate" appears here and in Transformer above.
# Expected: 2 results when querying "validate" -> scope=validate, Transformer.validate
def validate():
    pass


# "Node" appears in two different scopes.
# Expected: 2 results when querying "Node" -> scope=Tree.Node, Graph.Node
class Tree:
    class Node:
        def visit(self):
            pass


class Graph:
    class Node:
        def connect(self, other: "Graph.Node"):
            pass
