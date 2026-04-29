-- Bug hunt fixture for Lua parser

-- 1. Global function
function global_func()
    return 1
end

-- 2. Local function
local function local_func()
    return 2
end

-- 3. Dot method
function MyClass.static_method()
    return 3
end

-- 4. Colon method
function MyClass:instance_method()
    return 4
end

-- 5. Multi-level dot
function app.models.create_user(name, email)
    return { name = name, email = email }
end

-- 6. Nested function inside global function
function outer_func()
    function inner_func()
        return 5
    end
end

-- 7. Local function nested inside global function
function outer_local_nested()
    local function helper()
        return 6
    end
end

-- 8. Deeply nested
function level1()
    function level2()
        local function level3()
            return 7
        end
    end
end

-- 9. Dot method with nested function
function Config.load()
    function parse()
        return 8
    end
end

-- 10. Colon method with nested function
function Config:save()
    function write_data()
        return 9
    end
end

-- 11. Function with varargs
function varargs_func(...)
    return ...
end

-- 12. Empty function body
function empty_func()
end

-- 13. Function inside if block inside function
function container()
    if true then
        function conditional_func()
            return 10
        end
    end
end

-- 14. Function inside do block
do
    function scoped_func()
        return 11
    end
end

-- 15. Mixed dot and colon: function a.b:c()
function myapp.handlers:on_request()
    return 12
end

-- 16. Multiple functions at same level
function alpha() return 1 end
function beta() return 2 end
function gamma() return 3 end

-- These should NOT be extracted as functions:
local x = 10
local y = function() return 1 end
local z = "function fake()"
