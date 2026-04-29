# Fixture: scope resolution edge cases
# Tests that peek correctly distinguishes definitions by scope path.

# peek func process -> 3 results: scope=process, Alpha.process, Beta.process
def process():
    pass


class Alpha:
    # peek func process -> 3 results: scope=process, Alpha.process, Beta.process
    def process(self):
        pass

    # peek func handle -> 2 results: scope=handle, Alpha.handle
    def handle(self):
        pass


class Beta:
    # peek func process -> 3 results: scope=process, Alpha.process, Beta.process
    def process(self):
        pass


# peek func handle -> 2 results: scope=handle, Alpha.handle
def handle():
    pass


class First:
    # peek class Item -> 2 results: scope=First.Item, Second.Item
    class Item:
        pass


class Second:
    # peek class Item -> 2 results: scope=First.Item, Second.Item
    class Item:
        pass


# peek func validate -> 2 results: scope=validate, Gamma.validate
def validate():
    pass


class Gamma:
    # peek func validate -> 2 results: scope=validate, Gamma.validate
    def validate(self):
        pass
