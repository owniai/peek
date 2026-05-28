# edge.rb — boundary behaviors: op-assignment, lowercase var exclusion,
# class inheritance signature, function-body NOT extracted, alias scope/signature,
# nested module/class scope, operator method scope

# ── Op-assignment as Var ──
fallback_path ||= "/tmp/default"

# ── Op-assignment as Const ──
DEFAULT_TIMEOUT ||= 30
CACHED_VALUE &&= true
TOTAL += 1

# ── Top-level const ──
DEBUG_MODE = false

# ── Top-level var ──
timeout_seconds = 30

# ── Class inheritance signature ──
class AppError < StandardError
  DEFAULT_CODE = 500
end

# ── Lowercase var NOT extracted in class body ──
class Container
  local_var = 1
end

# ── Method body NOT recursed ──
def helper
  temp = 42
end

# ── Function-body definitions NOT extracted ──
def factory
  local_count = 0
  def inner_method
  end
end

# ── Nested module/class scope ──
module Outer
  module Inner
    class Deep
      def deep_method
      end
    end
  end
end

# ── Operator method scope ──
class Vector
  def ==(other)
    false
  end
end

# ── Alias scope/signature ──
class Config
  alias current default_config
  alias_method :alternate, :current
end

# ── Call block recursion: refine ──
module Extensions
  refine String do
    def shout
      upcase
    end
  end
end

# ── Call block recursion: each block with def (noise case) ──
items.each do |item|
  def helper
  end
end

# ── Struct.new with block: method inside struct body ──
Point3D = Struct.new(:x, :y, :z) do
  def to_s
    "#{x}, #{y}, #{z}"
  end
end

# ── define_method / define_singleton_method metaprogramming ──
class DynamicMethods
  define_method(:greet) { |name| "Hello #{name}" }

  define_singleton_method(:run) do
    puts "running"
  end
end