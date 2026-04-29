# Async function definitions fixture for tree-sitter-python parsing.
# tree-sitter uses "function_definition" for both sync and async def.

# peek func async_fetch → py/function, scope=async_fetch
async def async_fetch(url):
    pass


# peek func async_typed → py/function, scope=async_typed
async def async_typed() -> None:
    pass


# peek func process → py/function, scope=AsyncService.process
# peek func start → py/function, scope=AsyncService.start
class AsyncService:
    async def process(self):
        pass

    async def start(self):
        pass


# peek func async_static → py/function, scope=AsyncHelper.async_static
class AsyncHelper:
    @staticmethod
    async def async_static():
        pass


# peek func create → py/function, scope=AsyncFactory.create
class AsyncFactory:
    @classmethod
    async def create(cls):
        pass
