# peek class SimpleClass → py/class, scope=SimpleClass
class SimpleClass:
    # peek func instance_method → py/function, scope=SimpleClass.instance_method
    def instance_method(self):
        pass

# peek class ChildClass → py/class, scope=ChildClass
class ChildClass(Parent):
    # peek func child_method → py/function, scope=ChildClass.child_method
    def child_method(self):
        pass

# peek class MultiInherit → py/class, scope=MultiInherit
class MultiInherit(A, B, C):
    # peek func combined_method → py/function, scope=MultiInherit.combined_method
    def combined_method(self):
        pass

# peek class StaticHolder → py/class, scope=StaticHolder
class StaticHolder:
    # peek func static_helper → py/function, scope=StaticHolder.static_helper
    @staticmethod
    def static_helper():
        pass

# peek class ClassMeta → py/class, scope=ClassMeta
class ClassMeta:
    # peek func factory_method → py/function, scope=ClassMeta.factory_method
    @classmethod
    def factory_method(cls):
        pass

# peek class EmptyClass → py/class, scope=EmptyClass
class EmptyClass: pass

# peek class InitializerClass → py/class, scope=InitializerClass
class InitializerClass:
    # peek func __init__ → py/function, scope=InitializerClass.__init__
    def __init__(self, name):
        self.name = name
