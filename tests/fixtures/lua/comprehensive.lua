-- Lua comprehensive test fixture for peek
-- Covers: global function, local function, dot method, colon method, multi-level dot, nested scope

-- Top-level global function
function initialize()
    print("Initializing...")
end

-- Top-level local function
local function validate()
    return true
end

-- Function with parameters
function add(a, b)
    return a + b
end

-- Dot method (table method)
function math_utils.square(x)
    return x * x
end

-- Colon method
function math_utils:cube(x)
    return x * x * x
end

-- Multi-level dot method
function app.models.create_user(name)
    return { name = name }
end

-- Nested function inside a function
function process()
    function step1()
        print("Step 1")
    end

    function step2()
        print("Step 2")
    end
end

-- Deeply nested functions
function server()
    function handle_request()
        function parse_body()
            return {}
        end
    end
end

-- Dot method with nested function
function config.load()
    function parse_file()
        return {}
    end
end

-- Colon method with nested function
function config:save()
    function write_file()
        return true
    end
end

-- Local variables (should NOT be extracted)
local x = 10
local y = function() return 1 end
local z = 42
