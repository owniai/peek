# peek func retried_func → py/function, scope=retried_func, signature从def行开始
@retry
def retried_func():
    pass


# peek func stacked_func → py/function, scope=stacked_func, signature从def行开始
@timeout(30)
@retry(max=3)
def stacked_func():
    pass


# peek func api_handler → py/function, scope=api_handler, signature从def行开始
@route("/api/users")
def api_handler():
    pass


# peek class SingletonClass → py/class, scope=SingletonClass, signature从class行开始
@singleton
class SingletonClass:
    pass


# peek class WebApp → py/class, scope=WebApp
class WebApp:
    # peek func home → py/method, scope=WebApp.home, signature从def行开始
    @route("/home")
    def home(self):
        pass

    # peek func cached_data → py/method, scope=WebApp.cached_data, signature从def行开始
    @staticmethod
    @cache
    def cached_data():
        pass


# peek func name → py/method, scope=Config.name, signature从def行开始
class Config:
    @property
    def name(self):
        return "default"
