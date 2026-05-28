-- core.luau — all kind classifications, scope paths, signature formats, nested scope
-- Each construct appears once; no duplicate coverage across core and edge.

-- ── Function ──
function top_func(x: string): string
    return x
end


local function local_validate(): boolean
    return true
end


-- ── Method (dot) ──
function math_utils.square(x: number): number
    return x * x
end


-- ── Method (colon) ──
function math_utils:cube(x: number): number
    return x * x * x
end


-- ── Getter (__index) ──
function Vector.__index(t, key)
    return rawget(t, key)
end


-- ── Setter (__newindex) ──
function Vector.__newindex(t, key, val)
    rawset(t, key, val)
end


-- ── Operator (__add) ──
function Vector.__add(a, b)
    return a.value + b.value
end


-- ── Operator (__call) ──
function Vector.__call(t, ...)
    return t.value
end


-- ── Destructor (__gc) ──
function Vector.__gc(t)
    print("gc")
end


-- ── Const ──
local MAX_RETRIES <const> = 3


-- ── Var (local) ──
local config_path = "/etc/app.conf"


-- ── Var (global assignment) ──
port = 8080


-- ── Field (table constructor) ──
local defaults = {
    name = "app",
    timeout = 30,
}


-- ── Field + Method in table constructor ──
local handlers = {
    process = function(data)
        return data
    end,
    count = 42,
}


-- ── Type alias (no export) ──
type Point = { x: number, y: number }


-- ── Type alias (export) ──
export type Config = { host: string, port: number }


-- ── Generic function ──
function identity<T>(value: T): T
    return value
end


-- ── Multi-level dot scope ──
function app.models.create_user(name: string)
    return { name = name }
end


-- ── Nested function scope ──
function outer_func()
    local function inner_func()
        return 1
    end
end
