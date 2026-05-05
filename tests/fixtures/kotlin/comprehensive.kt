package com.example.testapp

import kotlin.math.abs
import java.util.List

// Top-level const
const val APP_NAME = "MyApp"
const val APP_VERSION = "1.0.0"
const val MAX_RETRIES = 3

// Type aliases
typealias StringMap = Map<String, String>
typealias Callback = (Int, String) -> Unit
typealias Processor<T> = (T) -> Boolean

// Top-level function
fun topLevelFunction(a: Int, b: String): Boolean {
    return a > 0 && b.isNotEmpty()
}

fun <T> genericFunction(item: T): List<T> {
    return listOf(item)
}

// Simple class
class SimpleClass(val name: String) {
    fun greet(): String = "Hello, $name"
}

// Class with secondary constructor
class Person(val firstName: String, val lastName: String) {
    constructor(fullName: String) : this(fullName.split(" ")[0], fullName.split(" ")[1])
}

// Data class
data class Point(val x: Double, val y: Double) {
    fun distanceTo(other: Point): Double {
        return Math.sqrt((x - other.x) * (x - other.x) + (y - other.y) * (y - other.y))
    }
}

// Sealed class
sealed class Shape {
    data class Circle(val radius: Double) : Shape()
    data class Rectangle(val width: Double, val height: Double) : Shape()
    object Empty : Shape()
}

// Abstract class
abstract class BaseProcessor {
    abstract fun process(input: String): String

    fun validate(input: String): Boolean {
        return input.isNotEmpty()
    }
}

// Annotation class
annotation class Serializable

// Interface
interface Drawable {
    fun draw()
    val area: Double
}

// Sealed interface
sealed interface Node {
    fun evaluate(): Int
    data class Literal(val value: Int) : Node {
        override fun evaluate(): Int = value
    }
    object Empty : Node {
        override fun evaluate(): Int = 0
    }
}

// Enum
enum class Color(val rgb: Int) {
    RED(0xFF0000),
    GREEN(0x00FF00),
    BLUE(0x0000FF);

    fun hex(): String = "#${rgb.toString(16).padStart(6, '0')}"
}

// Object declaration
object MathUtils {
    const val PI = 3.14159

    fun square(n: Int): Int = n * n
    fun cube(n: Int): Int = n * n * n
}

// Class with companion object
class Config {
    companion object Defaults {
        const val DEFAULT_TIMEOUT = 30
        const val DEFAULT_HOST = "localhost"

        fun create(): Config = Config()
    }

    var timeout: Int = Defaults.DEFAULT_TIMEOUT
    var host: String = Defaults.DEFAULT_HOST
}

// Class with anonymous companion object
class Repository {
    companion object {
        const val MAX_RESULTS = 100

        fun newInstance(): Repository = Repository()
    }
}

// Nested types
class Outer {
    class Inner {
        fun innerMethod() {}
    }

    interface Handler {
        fun handle(event: String)
    }

    object Cache {
        const val SIZE = 256

        fun clear() {}
    }

    enum class Priority {
        LOW, MEDIUM, HIGH
    }
}

// Deeply nested
class Container {
    class Builder {
        class Config {
            fun configure() {}
        }
    }
}

// Properties in class body
class UserProfile {
    var displayName: String = ""
    val isActive: Boolean = true
    var loginCount: Int = 0

    fun update() {}
}

// Class with mixed constructor params
class MixedParams(
    val propParam: String,
    var mutableParam: Int,
    plainParam: Double
) {
    fun method() {}
}
