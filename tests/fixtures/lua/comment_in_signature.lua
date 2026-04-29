-- Test fixture for Lua parser bug: comments leak into function signatures
-- Bug: comments between function header and body statements are included in the signature

-- Case 1: Comment inside function body before first statement
function greet()
    -- This comment should not appear in the signature
    local msg = "hello"
    return msg
end

-- Case 2: Comment between parameters and first statement
function calculate(a, b)
    -- Compute the result
    return a + b
end

-- Case 3: Local function with comment in body
local function helper()
    -- A helper comment
    return 42
end

-- Case 4: Method with comment in body
function Utils:format()
    -- Format the output
    return "formatted"
end

-- Case 5: Function with actual code on first body line (no comment)
function clean_func()
    return "clean"
end
