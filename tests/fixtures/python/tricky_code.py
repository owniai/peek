"""Fixture: parser resilience against false-positive definitions.

tree-sitter-python builds a full AST, so `def`/`class` tokens inside
string_literal or comment nodes are never mistaken for real definitions.
"""

# Real definition - should be found
# peek func real_func → 1 result
def real_func():
    pass


# Multiline signature - start_line must be the `def` line
# peek func multiline_params → 1 result (py/function, start_line = def line)
def multiline_params(
    alpha: int,
    beta: str,
    gamma: float,
) -> bool:
    pass


# Extra-long single-line signature
# peek func very_long_signature → 1 result
def very_long_signature(self, a: int, b: str, c: float, d: list, e: dict, f: tuple, g: set, h: Optional[str] = None, i: Union[int, str] = 0) -> dict[str, Any]:
    pass


# String containing a fake definition - tree-sitter sees string_literal, not function_definition
# peek func fake_in_string → 0 results (not a real definition, inside string)
code_string = "def fake_in_string():\n    pass"


# Comment containing a fake class - tree-sitter sees comment, not class_definition
# peek class FakeInComment → 0 results (not a real definition, inside comment)
# class FakeInComment:
#     pass


# Triple-quoted string containing fake definitions - still string_literal
# peek func fake_in_triple_quotes → 0 results (not a real definition, inside string)
# peek class AlsoFake → 0 results (not a real definition, inside string)
doc = '''
def fake_in_triple_quotes():
    pass
class AlsoFake:
    pass
'''


# Minimal class on one line
# peek class MinimalClass → 1 result
class MinimalClass: pass
