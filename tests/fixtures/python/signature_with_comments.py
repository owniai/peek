# Fixture: function/class definitions with inline comments in signatures.
# Tests that `strip_trailing_comment` doesn't truncate the signature when
# `flatten_bytes` merges multiple lines into one.


# Case 1: multiline parameters with trailing comments
def process_data(
    items: list,  # input items
    verbose: bool,  # verbose mode
) -> None:
    pass


# Case 2: decorated function with comment on decorator line
@retry(max_attempts=3)  # retry up to 3 times
def fetch_url(url: str) -> bytes:
    pass


# Case 3: stacked decorators with comments
@cache  # cache the result
@timeout(30)  # 30 second timeout
def compute(x: int) -> int:
    return x


# Case 4: decorated class with comment
@dataclass  # auto-generate __init__
class UserRecord:
    name: str
    age: int


# Case 5: multiline type alias
type Matrix = list[
    list[float],
]


# Case 6: class method with multiline params and comments
class Service:
    def handle_request(
        self,
        request: dict,  # incoming request
        timeout: float,  # max wait time
    ) -> bool:
        return True
