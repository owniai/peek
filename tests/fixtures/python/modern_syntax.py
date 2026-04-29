# Modern Python syntax fixture for tree-sitter parsing tests
# All types used here are placeholders — no real imports needed.

# 1. dataclass
# peek class UserRecord -> py/class, scope=UserRecord
@dataclass
class UserRecord:
    name: str
    age: int

# 2. Enum class
# peek class Color -> py/class, scope=Color
class Color(Enum):
    RED = 1
    GREEN = 2

# 3. Protocol class
# peek class Serializable -> py/class, scope=Serializable
class Serializable(Protocol):
    # peek func serialize -> py/function, scope=Serializable.serialize
    def serialize(self) -> bytes: ...

# 4. Complex type annotations
# peek func complex_types -> py/function, scope=complex_types
def complex_types(items: list[int], mapping: dict[str, float]) -> Optional[str]:
    pass

# 5. Generic class
# peek class Container -> py/class, scope=Container
class Container(Generic[T]):
    # peek func get -> py/function, scope=Container.get
    def get(self) -> T: ...

# 6. TypedDict class
# peek class Point -> py/class, scope=Point
class Point(TypedDict):
    x: float
    y: float
