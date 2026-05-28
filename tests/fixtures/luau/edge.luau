-- edge.luau — edge cases: single-line table fields, multiline type alias, export type function

-- ── Single-line table fields with metamethods ──
local mt = {
    __add = function(a, b) return a + b end,
    __index = function(t, k) return rawget(t, k) end,
    __newindex = function(t, k, v) rawset(t, k, v) end,
    __gc = function(t) print("gc") end,
    __tostring = function(t) return tostring(t) end,
    __call = function(t, ...) return t end,
    helper = function() return 1 end,
}

-- ── Function assigned to variable ──
local generate_id = function()
    return math.random(1, 1000)
end

global_handler = function(event)
    print(event)
end

-- ── Nested table with scope ──
local config = {
    db = {
        host = "localhost",
        port = 5432,
    },
    server = {
        name = "api",
    },
}

-- ── Generic type alias ──
type Result<T> = { ok: boolean, value: T }

-- ── Export type alias with generics ──
export type Predicate<T> = (T) -> boolean

-- ── Function declarations (no local) ──
function greet(): string
    return "hello"
end

function calculate(a: number, b: number): number
    return a + b
end

-- ── Table constructor as sole value ──
local items = {
    key = "value",
}

-- ── Function inside function body ──
function setup()
    local function teardown()
        print("done")
    end
end
