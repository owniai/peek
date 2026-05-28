# core.rb — all kind classifications, scope paths, signature formats
# Each construct appears once; no duplicate coverage across core and edge.

# ── Var (top-level) ──
config_path = '/etc/app'

# ── Const (top-level) ──
APP_VERSION = "1.0.0"

# ── Method (top-level) ──
def global_helper(x, y)
  x + y
end

# ── Singleton Method (top-level) ──
def self.configure(&block)
  block.call
end

# ── Module with nested class ──
module MyApp
  class User
    # ── Getter ──
    attr_reader :name

    # ── Setter ──
    attr_writer :age

    # ── Property ──
    attr_accessor :address

    # ── Constructor ──
    def initialize(name)
      @name = name
    end

    # ── Method ──
    def display_name
      "#{@name}"
    end

    # ── Alias ──
    alias full_name display_name

    # ── Singleton Method ──
    def self.find(email)
    end

    # ── Const (class scope) ──
    DEFAULT_ROLE = "member"
  end
end

# ── Module with only singleton methods ──
module Utilities
  def self.format_date(date)
    date.strftime("%Y-%m-%d")
  end

  def self.parse_json(str)
  end
end

# ── Class with inheritance ──
class ApplicationError < StandardError
  def initialize(message = "An error occurred")
    super(message)
  end

  DEFAULT_CODE = 500
end

# ── Class with nested class ──
class Container
  class Item
    def initialize(value)
      @value = value
    end

    def validate
      @value > 0
    end
  end

  MAX_ITEMS = 100
end

# ── Operator ──
class Vec2d
  def +(other)
  end

  def ==(other)
  end

  def [](index)
  end
end

# ── Struct (Struct.new) ──
Point = Struct.new(:x, :y)

# ── Struct (Data.define) ──
Coord = Data.define(:lat, :lng)