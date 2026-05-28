-- edge.lua — boundary behaviors: metamethod classification in table constructors,
-- anonymous function assignment as Function (not Var), multi-level dot scope,
-- nested table constructor scope, body comments in signature,
-- positional field exclusion, function-body definition exclusion

-- ── Table constructor metamethod classification ──
local mt = {
    __add = function(a, b) return a + b end,
    __index = function(t, k) return rawget(t, k) end,
    __newindex = function(t, k, v) rawset(t, k, v) end,
    __gc = function(t) print("gc") end,
    __tostring = function(t) return tostring(t) end,
    __call = function(t, ...) return t end,
    helper = function() return 1 end,
}


-- ── Anonymous function assignment as Function (not Var) ──
local generate_id = function()
    return math.random(1, 1000000)
end


global_handler = function(event)
    return event
end


-- ── Multi-level dot scope ──
function app.services.queue_task(task)
    return task
end


-- ── Nested table constructor scope ──
local config = {
    db = {
        host = "localhost",
        port = 5432,
    },
    server = {
        name = "api",
    },
}


-- ── Body comments in signature ──
function greet()
    -- This comment should not appear in the signature
    local msg = "hello"
    return msg
end


function calculate(a, b)
    -- Compute the result
    return a + b
end


-- ── Positional field not extracted ──
local items = {
    "positional",
    key = "value",
}


-- ── Function-body definitions NOT extracted ──
function setup()
    local temp = "inside"
    status = "done"
end