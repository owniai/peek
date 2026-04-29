# peek func simple_func → py/function, scope=simple_func
def simple_func():
    pass


# peek func multi_param → py/function, scope=multi_param
def multi_param(a, b, c):
    return a + b + c


# peek func typed_func → py/function, scope=typed_func
def typed_func(x: int) -> str:
    return str(x)


# peek func single_line_func → py/function, scope=single_line_func
def single_line_func():
    return 42


# peek func _private_helper → py/function, scope=_private_helper
def _private_helper():
    pass


# peek func __dunder_special__ → py/function, scope=__dunder_special__
def __dunder_special__():
    pass


# peek func default_params → py/function, scope=default_params
def default_params(x=10, y="hello"):
    return y * x
