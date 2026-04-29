# Ruby comprehensive test fixture for peek
# Covers: module, class, method, singleton_method, const, nested scope

# Top-level module with nested class, method, and const
module MyApp
  module Models
    class User
      attr_reader :name, :email

      def initialize(name, email)
        @name = name
        @email = email
      end

      def display_name
        "#{@name} <#{@email}>"
      end

      def self.find_by_email(email)
        # lookup logic
      end

      DEFAULT_ROLE = "member"
      MAX_LOGIN_ATTEMPTS = 5
    end
  end

  module Services
    class EmailService
      def send(to, subject, body)
        # send email
      end

      def self.default_sender
        "noreply@example.com"
      end

      SMTP_PORT = 587
    end
  end
end

# Top-level class with inheritance
class ApplicationError < StandardError
  def initialize(message = "An error occurred")
    super(message)
  end

  DEFAULT_CODE = 500
end

# Top-level methods
def global_helper(x, y)
  x + y
end

def self.configure(&block)
  block.call
end

# Top-level constants
APP_VERSION = "1.0.0"
DEBUG_MODE = false

# Module with only methods
module Utilities
  def self.format_date(date)
    date.strftime("%Y-%m-%d")
  end

  def self.parse_json(str)
    # parse logic
  end
end

# Nested class inside class
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
