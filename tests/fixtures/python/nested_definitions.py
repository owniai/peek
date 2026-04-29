# Fixture: nested class definitions for scope-building tests

# --- 2-layer nested class ---

# peek class OuterA → py/class, scope=OuterA
class OuterA:
    # peek class InnerA → py/class, scope=OuterA.InnerA
    class InnerA:
        pass

# --- Nested class with method ---

# peek class OuterB → py/class, scope=OuterB
class OuterB:
    # peek class InnerB → py/class, scope=OuterB.InnerB
    class InnerB:
        # peek func inner_method → py/function, scope=OuterB.InnerB.inner_method
        def inner_method(self):
            pass

# --- 5-layer deep nested class (key test point) ---

# peek class Level1 → py/class, scope=Level1
class Level1:
    # peek class Level2 → py/class, scope=Level1.Level2
    class Level2:
        # peek class Level3 → py/class, scope=Level1.Level2.Level3
        class Level3:
            # peek class Level4 → py/class, scope=Level1.Level2.Level3.Level4
            class Level4:
                # peek class Level5 → py/class, scope=Level1.Level2.Level3.Level4.Level5
                class Level5:
                    # peek func deep_method → py/function, scope=Level1.Level2.Level3.Level4.Level5.deep_method
                    def deep_method(self):
                        pass

# --- Multiple sibling nested classes inside one outer class ---

# peek class Container → py/class, scope=Container
class Container:
    # peek class PartA → py/class, scope=Container.PartA
    class PartA:
        pass

    # peek class PartB → py/class, scope=Container.PartB
    class PartB:
        pass
